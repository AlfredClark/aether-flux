use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, ensure, Context, Result};
use pinyin::ToPinyin;
use redb::{
    Database, MultimapTableDefinition, ReadableDatabase, ReadableMultimapTable, ReadableTable,
    ReadableTableMetadata, TableDefinition,
};
use serde::{Deserialize, Serialize};
use tauri::{async_runtime, AppHandle, Manager, State};

const WORDBANK_FILENAME: &str = "wordbank.redb";
const WORDBANK_BACKUP_DIRNAME: &str = "backups";
const WORDBANK_META_TABLE: TableDefinition<&str, &str> = TableDefinition::new("wordbank_meta");
const DEFAULT_WORDBANK_ID: &str = "default";
const DEFAULT_WORDBANK_NAME: &str = "Default";

/// 词库运行时状态，内部持有一个惰性初始化的 redb 加载器。
#[derive(Default)]
pub struct WordbankState {
    pub(crate) inner: Arc<Mutex<Option<WordbankLoader>>>,
}

/// 词库元信息。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbankSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub sort_order: u64,
    pub is_default: bool,
    pub is_enabled: bool,
    pub entry_total: usize,
}

/// 面向前端返回的词条分组条目。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbankEntry {
    pub key: String,
    pub values: Vec<String>,
}

/// 面向前端返回的分词同音候选项。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbankTokenHomophoneOptions {
    pub token: String,
    pub options: Vec<String>,
}

/// 用于 ASR 词库拟合器的启用词条。
#[derive(Debug, Clone)]
pub struct WordbankFitterEntry {
    pub key: String,
    pub value: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

/// 词条列表查询结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbankListResult {
    pub entries: Vec<WordbankEntry>,
    pub total: usize,
}

/// 批量导入词条的结果统计。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbankBatchResult {
    pub accepted_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WordbankBackupResult {
    pub backup_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordbankExportPackage {
    version: u32,
    exported_at_unix_seconds: u64,
    wordbanks: Vec<WordbankExportBank>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WordbankExportBank {
    name: String,
    description: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    sort_order: u64,
    is_default: bool,
    is_enabled: bool,
    entries: Vec<WordbankEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WordbankMetaRecord {
    name: String,
    description: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    #[serde(default)]
    sort_order: u64,
    is_default: bool,
    is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WordbankEntryOrderRecord {
    values: Vec<String>,
}

/// redb 词库加载器。
pub(crate) struct WordbankLoader {
    db: Database,
    table_name_cache: HashMap<String, (&'static str, &'static str, &'static str)>,
}

impl WordbankLoader {
    /// 打开或创建词库数据库，并确保元表与默认词库存在。
    fn open(path: PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

        let db = Database::create(&path)
            .with_context(|| format!("failed to open wordbank database {}", path.display()))?;
        let mut loader = Self {
            db,
            table_name_cache: HashMap::new(),
        };
        loader.initialize()?;
        Ok(loader)
    }

    fn initialize(&mut self) -> Result<()> {
        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        write_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;
        let table_names = self.table_names(DEFAULT_WORDBANK_ID);
        ensure_wordbank_tables(&write_txn, table_names.0, table_names.1, table_names.2)?;

        {
            let mut meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to reopen wordbank metadata table")?;
            if meta_table
                .get(DEFAULT_WORDBANK_ID)
                .context("failed to query default wordbank metadata")?
                .is_none()
            {
                let record = WordbankMetaRecord {
                    name: DEFAULT_WORDBANK_NAME.to_string(),
                    description: None,
                    prefix: None,
                    suffix: None,
                    sort_order: 0,
                    is_default: true,
                    is_enabled: true,
                };
                meta_table
                    .insert(DEFAULT_WORDBANK_ID, encode_meta_record(&record)?.as_str())
                    .context("failed to insert default wordbank metadata")?;
            }
        }

        write_txn
            .commit()
            .context("failed to initialize wordbank tables")?;
        Ok(())
    }

    /// 列出全部词库。
    fn list_wordbanks(&mut self) -> Result<Vec<WordbankSummary>> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        let meta_table = read_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;

        let mut banks = Vec::new();
        let mut iter = meta_table
            .iter()
            .context("failed to iterate wordbank metadata")?;
        while let Some(item) = iter.next() {
            let entry = item.context("failed to read wordbank metadata entry")?;
            let id = entry.0.value().to_string();
            let record = decode_meta_record(entry.1.value())?;
            let entry_total = self.count_entries_for_bank(&read_txn, &id)?;
            banks.push(WordbankSummary {
                id,
                name: record.name,
                description: record.description,
                prefix: record.prefix,
                suffix: record.suffix,
                sort_order: record.sort_order,
                is_default: record.is_default,
                is_enabled: record.is_enabled,
                entry_total,
            });
        }

        banks.sort_by(|left, right| sort_wordbank_summary(left, right));
        Ok(banks)
    }

    /// 创建一个新的词库。
    fn create_wordbank(
        &mut self,
        name: &str,
        description: Option<&str>,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> Result<WordbankSummary> {
        let record = WordbankMetaRecord {
            name: normalize_bank_name(name)?,
            description: normalize_description(description),
            prefix: normalize_affix(prefix),
            suffix: normalize_affix(suffix),
            sort_order: self.next_non_default_sort_order()?,
            is_default: false,
            is_enabled: true,
        };
        let id = generate_wordbank_id();

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        let table_names = self.table_names(&id);
        ensure_wordbank_tables(&write_txn, table_names.0, table_names.1, table_names.2)?;
        {
            let mut meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to open wordbank metadata table")?;
            meta_table
                .insert(id.as_str(), encode_meta_record(&record)?.as_str())
                .with_context(|| format!("failed to insert wordbank metadata for '{id}'"))?;
        }
        write_txn
            .commit()
            .context("failed to commit wordbank creation")?;

        Ok(WordbankSummary {
            id,
            name: record.name,
            description: record.description,
            prefix: record.prefix,
            suffix: record.suffix,
            sort_order: record.sort_order,
            is_default: false,
            is_enabled: true,
            entry_total: 0,
        })
    }

    /// 更新词库名称与简介。
    fn update_wordbank(
        &mut self,
        wordbank_id: &str,
        name: &str,
        description: Option<&str>,
        prefix: Option<&str>,
        suffix: Option<&str>,
    ) -> Result<WordbankSummary> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let updated_name = normalize_bank_name(name)?;
        let updated_description = normalize_description(description);
        let updated_prefix = normalize_affix(prefix);
        let updated_suffix = normalize_affix(suffix);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        let record = {
            let mut record = {
                let meta_table = write_txn
                    .open_table(WORDBANK_META_TABLE)
                    .context("failed to open wordbank metadata table")?;
                let existing = meta_table
                    .get(wordbank_id.as_str())
                    .with_context(|| format!("failed to query wordbank '{wordbank_id}'"))?
                    .ok_or_else(|| anyhow!("wordbank '{wordbank_id}' was not found"))?;
                decode_meta_record(existing.value())?
            };
            record.name = updated_name;
            record.description = updated_description;
            record.prefix = updated_prefix;
            record.suffix = updated_suffix;
            let mut meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to reopen wordbank metadata table")?;
            meta_table
                .insert(wordbank_id.as_str(), encode_meta_record(&record)?.as_str())
                .with_context(|| format!("failed to update wordbank '{wordbank_id}'"))?;
            record
        };
        write_txn
            .commit()
            .context("failed to commit wordbank update")?;

        let entry_total = self.count_entries(&wordbank_id)?;
        Ok(WordbankSummary {
            id: wordbank_id,
            name: record.name,
            description: record.description,
            prefix: record.prefix,
            suffix: record.suffix,
            sort_order: record.sort_order,
            is_default: record.is_default,
            is_enabled: record.is_enabled,
            entry_total,
        })
    }

    fn set_wordbank_enabled(
        &mut self,
        wordbank_id: &str,
        enabled: bool,
    ) -> Result<WordbankSummary> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        let record = {
            let mut record = {
                let meta_table = write_txn
                    .open_table(WORDBANK_META_TABLE)
                    .context("failed to open wordbank metadata table")?;
                let existing = meta_table
                    .get(wordbank_id.as_str())
                    .with_context(|| format!("failed to query wordbank '{wordbank_id}'"))?
                    .ok_or_else(|| anyhow!("wordbank '{wordbank_id}' was not found"))?;
                decode_meta_record(existing.value())?
            };
            if record.is_default {
                ensure!(enabled, "default wordbank cannot be disabled");
            }
            record.is_enabled = enabled;
            let mut meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to reopen wordbank metadata table")?;
            meta_table
                .insert(wordbank_id.as_str(), encode_meta_record(&record)?.as_str())
                .with_context(|| {
                    format!("failed to update wordbank enabled state for '{wordbank_id}'")
                })?;
            record
        };
        write_txn
            .commit()
            .context("failed to commit wordbank enabled state update")?;

        let entry_total = self.count_entries(&wordbank_id)?;
        Ok(WordbankSummary {
            id: wordbank_id,
            name: record.name,
            description: record.description,
            prefix: record.prefix,
            suffix: record.suffix,
            sort_order: record.sort_order,
            is_default: record.is_default,
            is_enabled: record.is_enabled,
            entry_total,
        })
    }

    fn reorder_wordbanks(&mut self, wordbank_ids: Vec<String>) -> Result<Vec<WordbankSummary>> {
        let normalized_ids = wordbank_ids
            .into_iter()
            .map(|id| normalize_wordbank_id(&id))
            .collect::<Result<Vec<_>>>()?;

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        let existing_records = {
            let meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to open wordbank metadata table")?;
            let mut records = Vec::<(String, WordbankMetaRecord)>::new();
            let mut iter = meta_table
                .iter()
                .context("failed to iterate wordbank metadata")?;
            while let Some(item) = iter.next() {
                let entry = item.context("failed to read wordbank metadata entry")?;
                records.push((
                    entry.0.value().to_string(),
                    decode_meta_record(entry.1.value())?,
                ));
            }
            records
        };

        let expected_ids = existing_records
            .iter()
            .filter(|(_, record)| !record.is_default)
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        let provided_ids = normalized_ids.iter().cloned().collect::<HashSet<_>>();
        ensure!(
            expected_ids == provided_ids,
            "reordered wordbanks must contain exactly all non-default wordbanks"
        );

        {
            let mut meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to reopen wordbank metadata table")?;
            for (index, wordbank_id) in normalized_ids.iter().enumerate() {
                let existing_value = meta_table
                    .get(wordbank_id.as_str())
                    .with_context(|| format!("failed to query wordbank '{wordbank_id}'"))?
                    .ok_or_else(|| anyhow!("wordbank '{wordbank_id}' was not found"))?
                    .value()
                    .to_string();
                let mut record = decode_meta_record(existing_value.as_str())?;
                ensure!(!record.is_default, "default wordbank cannot be reordered");
                record.sort_order = (index as u64) + 1;
                meta_table
                    .insert(wordbank_id.as_str(), encode_meta_record(&record)?.as_str())
                    .with_context(|| {
                        format!("failed to update wordbank order for '{wordbank_id}'")
                    })?;
            }
        }

        write_txn
            .commit()
            .context("failed to commit wordbank reorder")?;
        self.list_wordbanks()
    }

    /// 删除一个非默认词库，并直接删除其对应的 redb 表。
    fn delete_wordbank(&mut self, wordbank_id: &str) -> Result<()> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        let record = {
            let meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to open wordbank metadata table")?;
            let existing = meta_table
                .get(wordbank_id.as_str())
                .with_context(|| format!("failed to query wordbank '{wordbank_id}'"))?
                .ok_or_else(|| anyhow!("wordbank '{wordbank_id}' was not found"))?;
            decode_meta_record(existing.value())?
        };
        ensure!(!record.is_default, "default wordbank cannot be deleted");

        {
            let mut meta_table = write_txn
                .open_table(WORDBANK_META_TABLE)
                .context("failed to reopen wordbank metadata table")?;
            meta_table
                .remove(wordbank_id.as_str())
                .with_context(|| format!("failed to remove wordbank '{wordbank_id}' metadata"))?;
        }

        let table_names = self.table_names(&wordbank_id);
        let deleted_words = delete_word_table(&write_txn, table_names.0)
            .with_context(|| format!("failed to delete word table for '{wordbank_id}'"))?;
        let deleted_homophones = delete_homophone_table(&write_txn, table_names.1)
            .with_context(|| format!("failed to delete homophone table for '{wordbank_id}'"))?;
        let deleted_order = delete_order_table(&write_txn, table_names.2)
            .with_context(|| format!("failed to delete order table for '{wordbank_id}'"))?;
        ensure!(
            deleted_words || deleted_homophones || deleted_order,
            "wordbank '{wordbank_id}' did not have any tables to delete"
        );

        write_txn
            .commit()
            .context("failed to commit wordbank deletion")?;
        Ok(())
    }

    /// 清空指定词库内容，但保留其表结构。
    fn clear_wordbank(&mut self, wordbank_id: &str) -> Result<()> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        let table_names = self.table_names(&wordbank_id);

        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        let word_keys = {
            let word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut keys = Vec::new();
            let mut iter = word_table
                .iter()
                .with_context(|| format!("failed to iterate word entries for '{wordbank_id}'"))?;
            while let Some(item) = iter.next() {
                let entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::AccessGuard<'_, &'static str>,
                ) = item.context("failed to read word entry")?;
                keys.push(entry.0.value().to_string());
            }
            keys
        };
        let homophone_keys = {
            let homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut keys = Vec::new();
            let mut iter = homophone_table
                .iter()
                .with_context(|| format!("failed to iterate homophones for '{wordbank_id}'"))?;
            while let Some(item) = iter.next() {
                let entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::MultimapValue<'_, &'static str>,
                ) = item.context("failed to read homophone entry")?;
                keys.push(entry.0.value().to_string());
            }
            keys
        };
        let order_keys = {
            let order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;
            let mut keys = Vec::new();
            let mut iter = order_table
                .iter()
                .with_context(|| format!("failed to iterate order entries for '{wordbank_id}'"))?;
            while let Some(item) = iter.next() {
                let entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::AccessGuard<'_, &'static str>,
                ) = item.context("failed to read order entry")?;
                keys.push(entry.0.value().to_string());
            }
            keys
        };

        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to reopen word table for '{wordbank_id}'"))?;
            for key in &word_keys {
                word_table
                    .remove(key.as_str())
                    .with_context(|| format!("failed to remove key '{key}'"))?;
            }
        }
        {
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to reopen homophone table for '{wordbank_id}'"))?;
            for key in &homophone_keys {
                homophone_table
                    .remove_all(key.as_str())
                    .with_context(|| format!("failed to clear homophones for key '{key}'"))?;
            }
        }
        {
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to reopen order table for '{wordbank_id}'"))?;
            for key in &order_keys {
                order_table
                    .remove(key.as_str())
                    .with_context(|| format!("failed to clear order for key '{key}'"))?;
            }
        }

        write_txn
            .commit()
            .context("failed to commit wordbank clearing")?;
        Ok(())
    }

    /// 按可选查询条件列出指定词库中的全部词条分组。
    fn list_entries(
        &mut self,
        wordbank_id: &str,
        query: Option<&str>,
    ) -> Result<WordbankListResult> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let table_names = self.table_names(&wordbank_id);
        let normalized_query = query
            .map(str::trim)
            .filter(|query| !query.is_empty())
            .map(|query| query.to_lowercase());

        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        ensure_wordbank_exists_in_read(&read_txn, &wordbank_id)?;
        let word_table = open_word_table_read(&read_txn, table_names.0)
            .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
        let homophone_table = open_homophone_table_read(&read_txn, table_names.1)
            .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
        let order_table = open_order_table_read(&read_txn, table_names.2)
            .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

        let mut entries = Vec::new();
        let mut iter = word_table
            .iter()
            .context("failed to iterate wordbank entries")?;
        while let Some(item) = iter.next() {
            let entry: (
                redb::AccessGuard<'_, &'static str>,
                redb::AccessGuard<'_, &'static str>,
            ) = item.context("failed to read wordbank entry")?;
            let key = entry.0.value().to_string();
            let values =
                load_ordered_values_read(&word_table, &homophone_table, &order_table, &key)?;
            let matches_query = normalized_query.as_ref().is_none_or(|query| {
                key.to_lowercase().contains(query)
                    || values
                        .iter()
                        .any(|value: &String| value.contains(query.as_str()))
            });

            if matches_query {
                entries.push(WordbankEntry { key, values });
            }
        }

        entries.sort_by(|left, right| left.key.cmp(&right.key));
        Ok(WordbankListResult {
            total: entries.len(),
            entries,
        })
    }

    /// 新增一个中文词条到指定词库。
    fn add_entry(&mut self, wordbank_id: &str, value: &str) -> Result<WordbankEntry> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let value = normalize_word(value)?;
        let key = build_pinyin_key(&value)?;
        let table_names = self.table_names(&wordbank_id);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;
            insert_value(
                &mut word_table,
                &mut homophone_table,
                &mut order_table,
                &key,
                &value,
            )?;
        }
        write_txn
            .commit()
            .context("failed to commit wordbank insertion")?;

        self.get_entry(&wordbank_id, &key)?
            .ok_or_else(|| anyhow!("wordbank entry disappeared after insertion"))
    }

    /// 按空白字符拆分文本并批量新增中文词条到指定词库。
    fn add_entries_from_text(
        &mut self,
        wordbank_id: &str,
        text: &str,
    ) -> Result<WordbankBatchResult> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let values = split_wordbank_text(text)?;
        let table_names = self.table_names(&wordbank_id);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            for value in &values {
                let key = build_pinyin_key(value)?;
                insert_value(
                    &mut word_table,
                    &mut homophone_table,
                    &mut order_table,
                    &key,
                    value,
                )?;
            }
        }
        write_txn
            .commit()
            .context("failed to commit wordbank batch insertion")?;

        Ok(WordbankBatchResult {
            accepted_count: values.len(),
        })
    }

    /// 更新一个已有词条，必要时会移动到新的拼音键分组。
    fn update_entry(
        &mut self,
        wordbank_id: &str,
        original_value: &str,
        new_value: &str,
    ) -> Result<WordbankEntry> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let original_value = normalize_word(original_value)?;
        let new_value = normalize_word(new_value)?;
        let original_key = build_pinyin_key(&original_value)?;
        let new_key = build_pinyin_key(&new_value)?;
        let table_names = self.table_names(&wordbank_id);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;
            remove_value(
                &mut word_table,
                &mut homophone_table,
                &mut order_table,
                &original_key,
                &original_value,
            )?;
            insert_value(
                &mut word_table,
                &mut homophone_table,
                &mut order_table,
                &new_key,
                &new_value,
            )?;
        }
        write_txn
            .commit()
            .context("failed to commit wordbank update")?;

        self.get_entry(&wordbank_id, &new_key)?
            .ok_or_else(|| anyhow!("wordbank entry disappeared after update"))
    }

    /// 删除一个词条；若它是某个键下的主值，则自动提升一个同音值为主值。
    fn delete_entry(&mut self, wordbank_id: &str, value: &str) -> Result<()> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let value = normalize_word(value)?;
        let key = build_pinyin_key(&value)?;
        let table_names = self.table_names(&wordbank_id);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;
            remove_value(
                &mut word_table,
                &mut homophone_table,
                &mut order_table,
                &key,
                &value,
            )?;
        }
        write_txn
            .commit()
            .context("failed to commit wordbank deletion")?;

        Ok(())
    }

    /// 读取单个拼音键对应的聚合条目。
    fn get_entry(&mut self, wordbank_id: &str, key: &str) -> Result<Option<WordbankEntry>> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        ensure_wordbank_exists_in_read(&read_txn, wordbank_id)?;
        let table_names = self.table_names(wordbank_id);
        let word_table = open_word_table_read(&read_txn, table_names.0)
            .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
        let homophone_table = open_homophone_table_read(&read_txn, table_names.1)
            .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
        let order_table = open_order_table_read(&read_txn, table_names.2)
            .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;
        if word_table
            .get(key)
            .with_context(|| format!("failed to query key '{key}'"))?
            .is_none()
        {
            return Ok(None);
        }
        let values = load_ordered_values_read(&word_table, &homophone_table, &order_table, key)?;

        Ok(Some(WordbankEntry {
            key: key.to_string(),
            values,
        }))
    }

    fn count_entries(&mut self, wordbank_id: &str) -> Result<usize> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        self.count_entries_for_bank(&read_txn, wordbank_id)
    }

    fn count_entries_for_bank(
        &mut self,
        read_txn: &redb::ReadTransaction,
        wordbank_id: &str,
    ) -> Result<usize> {
        ensure_wordbank_exists_in_read(read_txn, wordbank_id)?;
        let table_names = self.table_names(wordbank_id);
        let word_table = open_word_table_read(read_txn, table_names.0)
            .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
        Ok(word_table
            .len()
            .with_context(|| format!("failed to count entries for '{wordbank_id}'"))?
            .try_into()
            .context("wordbank entry count overflowed usize")?)
    }

    fn delete_entry_group(&mut self, wordbank_id: &str, key: &str) -> Result<()> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let key = normalize_entry_key(key)?;
        let table_names = self.table_names(&wordbank_id);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            ensure!(
                word_table
                    .get(key.as_str())
                    .with_context(|| format!("failed to query key '{key}'"))?
                    .is_some(),
                "wordbank entry group '{key}' was not found"
            );
            word_table
                .remove(key.as_str())
                .with_context(|| format!("failed to remove key '{key}'"))?;
            homophone_table
                .remove_all(key.as_str())
                .with_context(|| format!("failed to clear homophones for key '{key}'"))?;
            order_table
                .remove(key.as_str())
                .with_context(|| format!("failed to remove order for key '{key}'"))?;
        }
        write_txn
            .commit()
            .context("failed to commit wordbank group deletion")?;
        Ok(())
    }

    fn reorder_entry_group(
        &mut self,
        wordbank_id: &str,
        key: &str,
        values: Vec<String>,
    ) -> Result<WordbankEntry> {
        let wordbank_id = normalize_wordbank_id(wordbank_id)?;
        let key = normalize_entry_key(key)?;
        let reordered_values = normalize_entry_group_values(values)?;
        let table_names = self.table_names(&wordbank_id);

        let write_txn = self
            .db
            .begin_write()
            .context("failed to open wordbank write txn")?;
        ensure_wordbank_exists_in_write(&write_txn, &wordbank_id)?;
        {
            let mut word_table = open_word_table_write(&write_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let mut order_table = open_order_table_write(&write_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            let existing_values =
                load_ordered_values_write(&word_table, &homophone_table, &order_table, &key)?;
            let existing_set = existing_values.iter().cloned().collect::<HashSet<_>>();
            let next_set = reordered_values.iter().cloned().collect::<HashSet<_>>();
            ensure!(
                existing_set == next_set,
                "reordered values must contain exactly the existing group entries"
            );

            homophone_table
                .remove_all(key.as_str())
                .with_context(|| format!("failed to clear homophones for key '{key}'"))?;
            word_table
                .insert(key.as_str(), reordered_values[0].as_str())
                .with_context(|| format!("failed to update primary value for key '{key}'"))?;
            for value in reordered_values.iter().skip(1) {
                homophone_table
                    .insert(key.as_str(), value.as_str())
                    .with_context(|| format!("failed to restore homophone value '{value}'"))?;
            }
            store_order_record(&mut order_table, &key, &reordered_values)?;
        }
        write_txn
            .commit()
            .context("failed to commit wordbank group reorder")?;

        self.get_entry(&wordbank_id, &key)?
            .ok_or_else(|| anyhow!("wordbank entry group disappeared after reorder"))
    }

    fn collect_enabled_words(&mut self) -> Result<Vec<String>> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        let meta_table = read_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;

        let mut words = Vec::new();
        let mut seen = HashSet::new();
        let mut iter = meta_table
            .iter()
            .context("failed to iterate wordbank metadata")?;
        while let Some(item) = iter.next() {
            let entry = item.context("failed to read wordbank metadata entry")?;
            let wordbank_id = entry.0.value().to_string();
            let record = decode_meta_record(entry.1.value())?;
            if !record.is_enabled {
                continue;
            }

            let table_names = self.table_names(&wordbank_id);
            let word_table = open_word_table_read(&read_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let homophone_table = open_homophone_table_read(&read_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let order_table = open_order_table_read(&read_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            let mut word_iter = word_table
                .iter()
                .with_context(|| format!("failed to iterate word entries for '{wordbank_id}'"))?;
            while let Some(word_item) = word_iter.next() {
                let word_entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::AccessGuard<'_, &'static str>,
                ) = word_item.context("failed to read wordbank word entry")?;
                let key = word_entry.0.value().to_string();
                for value in
                    load_ordered_values_read(&word_table, &homophone_table, &order_table, &key)?
                {
                    if seen.insert(value.clone()) {
                        words.push(value);
                    }
                }
            }
        }

        Ok(words)
    }

    fn collect_enabled_entry_groups(&mut self) -> Result<Vec<WordbankEntry>> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        let meta_table = read_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;

        let mut metas = Vec::<(String, WordbankMetaRecord)>::new();
        let mut iter = meta_table
            .iter()
            .context("failed to iterate wordbank metadata")?;
        while let Some(item) = iter.next() {
            let entry = item.context("failed to read wordbank metadata entry")?;
            let wordbank_id = entry.0.value().to_string();
            let record = decode_meta_record(entry.1.value())?;
            if !record.is_enabled {
                continue;
            }
            metas.push((wordbank_id, record));
        }

        metas.sort_by(|left, right| sort_wordbank_meta_entry(left, right));

        let mut groups = Vec::<WordbankEntry>::new();
        let mut group_index_by_key = HashMap::<String, usize>::new();
        for (wordbank_id, _) in metas {
            let table_names = self.table_names(&wordbank_id);
            let word_table = open_word_table_read(&read_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let homophone_table = open_homophone_table_read(&read_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let order_table = open_order_table_read(&read_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            let mut word_iter = word_table
                .iter()
                .with_context(|| format!("failed to iterate word entries for '{wordbank_id}'"))?;
            while let Some(word_item) = word_iter.next() {
                let word_entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::AccessGuard<'_, &'static str>,
                ) = word_item.context("failed to read wordbank word entry")?;
                let key = word_entry.0.value().to_string();
                let values =
                    load_ordered_values_read(&word_table, &homophone_table, &order_table, &key)?;

                let group_index = if let Some(index) = group_index_by_key.get(&key) {
                    *index
                } else {
                    let index = groups.len();
                    groups.push(WordbankEntry {
                        key: key.clone(),
                        values: Vec::new(),
                    });
                    group_index_by_key.insert(key.clone(), index);
                    index
                };

                let group = &mut groups[group_index];
                for value in values {
                    if !group.values.iter().any(|existing| existing == &value) {
                        group.values.push(value);
                    }
                }
            }
        }

        Ok(groups)
    }

    fn collect_enabled_fitter_entries(&mut self) -> Result<Vec<WordbankFitterEntry>> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        let meta_table = read_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;

        let mut metas = Vec::<(String, WordbankMetaRecord)>::new();
        let mut iter = meta_table
            .iter()
            .context("failed to iterate wordbank metadata")?;
        while let Some(item) = iter.next() {
            let entry = item.context("failed to read wordbank metadata entry")?;
            let wordbank_id = entry.0.value().to_string();
            let record = decode_meta_record(entry.1.value())?;
            if record.is_enabled {
                metas.push((wordbank_id, record));
            }
        }

        metas.sort_by(|left, right| sort_wordbank_meta_entry(left, right));

        let mut entries = Vec::new();
        for (wordbank_id, record) in metas {
            let table_names = self.table_names(&wordbank_id);
            let word_table = open_word_table_read(&read_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let homophone_table = open_homophone_table_read(&read_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let order_table = open_order_table_read(&read_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            let mut word_iter = word_table
                .iter()
                .with_context(|| format!("failed to iterate word entries for '{wordbank_id}'"))?;
            while let Some(word_item) = word_iter.next() {
                let word_entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::AccessGuard<'_, &'static str>,
                ) = word_item.context("failed to read wordbank word entry")?;
                let key = word_entry.0.value().to_string();
                let Some(value) =
                    load_ordered_values_read(&word_table, &homophone_table, &order_table, &key)?
                        .into_iter()
                        .next()
                else {
                    continue;
                };
                entries.push(WordbankFitterEntry {
                    key,
                    value,
                    prefix: record.prefix.clone(),
                    suffix: record.suffix.clone(),
                });
            }
        }

        Ok(entries)
    }

    fn table_names(&mut self, wordbank_id: &str) -> (&'static str, &'static str, &'static str) {
        let names = self
            .table_name_cache
            .entry(wordbank_id.to_string())
            .or_insert_with(|| {
                (
                    Box::leak(word_table_name(wordbank_id).into_boxed_str()) as &'static str,
                    Box::leak(homophone_table_name(wordbank_id).into_boxed_str()) as &'static str,
                    Box::leak(order_table_name(wordbank_id).into_boxed_str()) as &'static str,
                )
            });
        *names
    }

    fn next_non_default_sort_order(&mut self) -> Result<u64> {
        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        let meta_table = read_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;
        let mut max_sort_order = 0u64;
        let mut iter = meta_table
            .iter()
            .context("failed to iterate wordbank metadata")?;
        while let Some(item) = iter.next() {
            let entry = item.context("failed to read wordbank metadata entry")?;
            let record = decode_meta_record(entry.1.value())?;
            if !record.is_default {
                max_sort_order = max_sort_order.max(record.sort_order);
            }
        }
        Ok(max_sort_order + 1)
    }

    fn export_wordbanks(&mut self, wordbank_ids: Vec<String>) -> Result<String> {
        let requested_ids = wordbank_ids
            .into_iter()
            .map(|id| normalize_wordbank_id(&id))
            .collect::<Result<HashSet<_>>>()?;
        ensure!(
            !requested_ids.is_empty(),
            "at least one wordbank must be selected for export"
        );

        let read_txn = self
            .db
            .begin_read()
            .context("failed to open wordbank read txn")?;
        let meta_table = read_txn
            .open_table(WORDBANK_META_TABLE)
            .context("failed to open wordbank metadata table")?;

        let mut exported = Vec::<WordbankExportBank>::new();
        let mut iter = meta_table
            .iter()
            .context("failed to iterate wordbank metadata")?;
        while let Some(item) = iter.next() {
            let entry = item.context("failed to read wordbank metadata entry")?;
            let wordbank_id = entry.0.value().to_string();
            if !requested_ids.contains(&wordbank_id) {
                continue;
            }

            let record = decode_meta_record(entry.1.value())?;
            let table_names = self.table_names(&wordbank_id);
            let word_table = open_word_table_read(&read_txn, table_names.0)
                .with_context(|| format!("failed to open word table for '{wordbank_id}'"))?;
            let homophone_table = open_homophone_table_read(&read_txn, table_names.1)
                .with_context(|| format!("failed to open homophone table for '{wordbank_id}'"))?;
            let order_table = open_order_table_read(&read_txn, table_names.2)
                .with_context(|| format!("failed to open order table for '{wordbank_id}'"))?;

            let mut entries = Vec::<WordbankEntry>::new();
            let mut word_iter = word_table
                .iter()
                .with_context(|| format!("failed to iterate word entries for '{wordbank_id}'"))?;
            while let Some(word_item) = word_iter.next() {
                let word_entry: (
                    redb::AccessGuard<'_, &'static str>,
                    redb::AccessGuard<'_, &'static str>,
                ) = word_item.context("failed to read wordbank word entry")?;
                let key = word_entry.0.value().to_string();
                let values =
                    load_ordered_values_read(&word_table, &homophone_table, &order_table, &key)?;
                entries.push(WordbankEntry { key, values });
            }
            entries.sort_by(|left, right| left.key.cmp(&right.key));

            exported.push(WordbankExportBank {
                name: record.name,
                description: record.description,
                prefix: record.prefix,
                suffix: record.suffix,
                sort_order: record.sort_order,
                is_default: record.is_default,
                is_enabled: record.is_enabled,
                entries,
            });
        }

        ensure!(
            exported.len() == requested_ids.len(),
            "some selected wordbanks were not found for export"
        );
        exported.sort_by(|left, right| {
            right
                .is_default
                .cmp(&left.is_default)
                .then_with(|| left.sort_order.cmp(&right.sort_order))
                .then_with(|| left.name.cmp(&right.name))
        });

        let exported_at_unix_seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or_default();

        serde_json::to_string_pretty(&WordbankExportPackage {
            version: 1,
            exported_at_unix_seconds,
            wordbanks: exported,
        })
        .context("failed to encode exported wordbanks")
    }

    fn import_wordbanks(&mut self, payload: &str) -> Result<Vec<WordbankSummary>> {
        let package: WordbankExportPackage =
            serde_json::from_str(payload).context("failed to decode imported wordbanks")?;
        ensure!(package.version == 1, "unsupported wordbank export version");
        ensure!(
            !package.wordbanks.is_empty(),
            "import payload did not contain any wordbanks"
        );

        let mut imported_ids = Vec::<String>::new();
        for bank in package.wordbanks {
            let imported_id = if bank.is_default {
                let wordbank_id = DEFAULT_WORDBANK_ID.to_string();
                self.clear_wordbank(&wordbank_id)?;
                let updated = self.update_wordbank(
                    &wordbank_id,
                    &bank.name,
                    bank.description.as_deref(),
                    bank.prefix.as_deref(),
                    bank.suffix.as_deref(),
                )?;
                self.set_wordbank_enabled(&updated.id, true)?;
                updated.id
            } else {
                let created = self.create_wordbank(
                    &bank.name,
                    bank.description.as_deref(),
                    bank.prefix.as_deref(),
                    bank.suffix.as_deref(),
                )?;
                self.set_wordbank_enabled(&created.id, bank.is_enabled)?;
                created.id
            };

            for entry in bank.entries {
                let normalized_values = normalize_entry_group_values(entry.values)?;
                let imported_id_for_entry = imported_id.clone();
                let table_names = self.table_names(&imported_id_for_entry);
                let write_txn = self
                    .db
                    .begin_write()
                    .context("failed to open wordbank write txn")?;
                ensure_wordbank_exists_in_write(&write_txn, &imported_id_for_entry)?;
                {
                    let mut word_table = open_word_table_write(&write_txn, table_names.0)
                        .with_context(|| {
                            format!("failed to open word table for '{imported_id_for_entry}'")
                        })?;
                    let mut homophone_table = open_homophone_table_write(&write_txn, table_names.1)
                        .with_context(|| {
                            format!("failed to open homophone table for '{imported_id_for_entry}'")
                        })?;
                    let mut order_table = open_order_table_write(&write_txn, table_names.2)
                        .with_context(|| {
                            format!("failed to open order table for '{imported_id_for_entry}'")
                        })?;
                    for value in &normalized_values {
                        let key = build_pinyin_key(value)?;
                        insert_value(
                            &mut word_table,
                            &mut homophone_table,
                            &mut order_table,
                            &key,
                            value,
                        )?;
                    }
                    let imported_key = normalize_entry_key(&entry.key)?;
                    let actual_values = load_ordered_values_write(
                        &word_table,
                        &homophone_table,
                        &order_table,
                        &imported_key,
                    )?;
                    ensure!(
                        actual_values.iter().cloned().collect::<HashSet<_>>()
                            == normalized_values.iter().cloned().collect::<HashSet<_>>(),
                        "imported entry values did not match pinyin key '{}'",
                        entry.key
                    );
                    homophone_table
                        .remove_all(imported_key.as_str())
                        .with_context(|| {
                            format!("failed to reset homophones for '{}'", entry.key)
                        })?;
                    word_table
                        .insert(imported_key.as_str(), normalized_values[0].as_str())
                        .with_context(|| {
                            format!("failed to restore primary value for '{}'", entry.key)
                        })?;
                    for value in normalized_values.iter().skip(1) {
                        homophone_table
                            .insert(imported_key.as_str(), value.as_str())
                            .with_context(|| {
                                format!("failed to restore ordered homophone value '{value}'")
                            })?;
                    }
                    store_order_record(&mut order_table, &imported_key, &normalized_values)?;
                }
                write_txn
                    .commit()
                    .context("failed to commit imported wordbank entry")?;
            }

            imported_ids.push(imported_id);
        }

        let mut banks = self.list_wordbanks()?;
        banks.retain(|bank| imported_ids.iter().any(|id| id == &bank.id));
        Ok(banks)
    }
}

/// 列出全部词库。
#[tauri::command]
pub async fn list_wordbanks(
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<Vec<WordbankSummary>, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| loader.list_wordbanks())
    })
    .await
    .map_err(|err| format!("Failed to join wordbank list task: {err}"))?
}

/// 创建词库。
#[tauri::command]
pub async fn create_wordbank(
    name: String,
    description: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankSummary, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.create_wordbank(
                &name,
                description.as_deref(),
                prefix.as_deref(),
                suffix.as_deref(),
            )
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank create task: {err}"))?
}

/// 更新词库元信息。
#[tauri::command]
pub async fn update_wordbank(
    wordbank_id: String,
    name: String,
    description: Option<String>,
    prefix: Option<String>,
    suffix: Option<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankSummary, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.update_wordbank(
                &wordbank_id,
                &name,
                description.as_deref(),
                prefix.as_deref(),
                suffix.as_deref(),
            )
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank update task: {err}"))?
}

/// 重排非默认词库优先级。
#[tauri::command]
pub async fn reorder_wordbanks(
    wordbank_ids: Vec<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<Vec<WordbankSummary>, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.reorder_wordbanks(wordbank_ids)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank reorder task: {err}"))?
}

/// 更新词库启用状态。
#[tauri::command]
pub async fn set_wordbank_enabled(
    wordbank_id: String,
    enabled: bool,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankSummary, String> {
    let app_for_update = app.clone();
    let state = Arc::clone(&wordbank_state.inner);
    let updated = async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app_for_update, &state, |loader| {
            loader.set_wordbank_enabled(&wordbank_id, enabled)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank enabled state task: {err}"))??;
    Ok(updated)
}

/// 删除词库。
#[tauri::command]
pub async fn delete_wordbank(
    wordbank_id: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| loader.delete_wordbank(&wordbank_id))
    })
    .await
    .map_err(|err| format!("Failed to join wordbank delete task: {err}"))?
}

/// 清空词库内容。
#[tauri::command]
pub async fn clear_wordbank(
    wordbank_id: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| loader.clear_wordbank(&wordbank_id))
    })
    .await
    .map_err(|err| format!("Failed to join wordbank clear task: {err}"))?
}

/// 查询指定词库的词条列表。
#[tauri::command]
pub async fn list_wordbank_entries(
    wordbank_id: String,
    query: Option<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankListResult, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.list_entries(&wordbank_id, query.as_deref())
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank entry list task: {err}"))?
}

/// 添加词条。
#[tauri::command]
pub async fn add_wordbank_entry(
    wordbank_id: String,
    value: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankEntry, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.add_entry(&wordbank_id, &value)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank add task: {err}"))?
}

/// 按空白分隔文本批量添加词条。
#[tauri::command]
pub async fn add_wordbank_entries_from_text(
    wordbank_id: String,
    text: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankBatchResult, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.add_entries_from_text(&wordbank_id, &text)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank batch add task: {err}"))?
}

/// 更新词条。
#[tauri::command]
pub async fn update_wordbank_entry(
    wordbank_id: String,
    original_value: String,
    new_value: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankEntry, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.update_entry(&wordbank_id, &original_value, &new_value)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank update task: {err}"))?
}

/// 删除词条。
#[tauri::command]
pub async fn delete_wordbank_entry(
    wordbank_id: String,
    value: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.delete_entry(&wordbank_id, &value)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank delete task: {err}"))?
}

/// 删除整组词条。
#[tauri::command]
pub async fn delete_wordbank_entry_group(
    wordbank_id: String,
    key: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.delete_entry_group(&wordbank_id, &key)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank group delete task: {err}"))?
}

/// 重排同一组内的词条顺序。
#[tauri::command]
pub async fn reorder_wordbank_entry_group(
    wordbank_id: String,
    key: String,
    values: Vec<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankEntry, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            loader.reorder_entry_group(&wordbank_id, &key, values)
        })
    })
    .await
    .map_err(|err| format!("Failed to join wordbank group reorder task: {err}"))?
}

/// 导出选中的词库为 JSON。
#[tauri::command]
pub async fn export_wordbanks(
    wordbank_ids: Vec<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<String, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| loader.export_wordbanks(wordbank_ids))
    })
    .await
    .map_err(|err| format!("Failed to join wordbank export task: {err}"))?
}

/// 从 JSON 导入词库。
#[tauri::command]
pub async fn import_wordbanks(
    payload: String,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<Vec<WordbankSummary>, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| loader.import_wordbanks(&payload))
    })
    .await
    .map_err(|err| format!("Failed to join wordbank import task: {err}"))?
}

/// 备份词库数据库文件。
#[tauri::command]
pub fn backup_wordbank_database(
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<WordbankBackupResult, String> {
    {
        let mut guard = wordbank_state
            .inner
            .lock()
            .map_err(|_| "Failed to lock wordbank state".to_string())?;
        *guard = None;
    }

    let source_path = resolve_wordbank_path(&app)
        .map_err(|err| format!("failed to resolve wordbank path: {err:#}"))?;
    if !source_path.exists() {
        return Err("failed to backup wordbank: wordbank database file was not found".to_string());
    }

    let backup_dir = resolve_wordbank_backup_dir(&app)
        .map_err(|err| format!("failed to resolve wordbank backup dir: {err:#}"))?;
    fs::create_dir_all(&backup_dir).map_err(|err| {
        format!(
            "failed to create backup dir {}: {err}",
            backup_dir.display()
        )
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();
    let backup_path = backup_dir.join(format!("wordbank-{timestamp}.redb"));
    fs::copy(&source_path, &backup_path).map_err(|err| {
        format!(
            "failed to copy wordbank database from {} to {}: {err}",
            source_path.display(),
            backup_path.display()
        )
    })?;

    Ok(WordbankBackupResult {
        backup_path: backup_path.to_string_lossy().into_owned(),
    })
}

/// 通过删除词库文件重置全部词库。
#[tauri::command]
pub fn reset_wordbank_database(
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<(), String> {
    {
        let mut guard = wordbank_state
            .inner
            .lock()
            .map_err(|_| "Failed to lock wordbank state".to_string())?;
        *guard = None;
    }

    let path = resolve_wordbank_path(&app)
        .map_err(|err| format!("failed to resolve wordbank path: {err:#}"))?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|err| format!("failed to remove wordbank file {}: {err}", path.display()))?;
    }
    Ok(())
}

pub(crate) fn collect_enabled_wordbank_words(
    app: &AppHandle,
    wordbank_state: &WordbankState,
) -> Result<Vec<String>, String> {
    let state = Arc::clone(&wordbank_state.inner);
    with_wordbank_loader(app, &state, |loader| loader.collect_enabled_words())
}

pub(crate) fn collect_enabled_wordbank_fitter_entries(
    app: &AppHandle,
    wordbank_state: &WordbankState,
) -> Result<Vec<WordbankFitterEntry>, String> {
    let state = Arc::clone(&wordbank_state.inner);
    with_wordbank_loader(app, &state, |loader| {
        loader.collect_enabled_fitter_entries()
    })
}

/// 查询一批分词结果在已启用词库中的同音候选项。
#[tauri::command]
pub async fn list_enabled_wordbank_homophones(
    tokens: Vec<String>,
    app: AppHandle,
    wordbank_state: State<'_, WordbankState>,
) -> Result<Vec<WordbankTokenHomophoneOptions>, String> {
    let state = Arc::clone(&wordbank_state.inner);
    async_runtime::spawn_blocking(move || {
        with_wordbank_loader(&app, &state, |loader| {
            let groups = loader.collect_enabled_entry_groups()?;
            let group_map = groups
                .into_iter()
                .map(|group| (group.key, group.values))
                .collect::<HashMap<_, _>>();

            Ok(tokens
                .into_iter()
                .map(|token| {
                    let mut options = build_pinyin_key(&token)
                        .ok()
                        .and_then(|key| group_map.get(&key).cloned())
                        .unwrap_or_default();

                    if options.len() <= 1 {
                        options.clear();
                    } else if !options.iter().any(|value| value == &token) {
                        options.insert(0, token.clone());
                    }

                    WordbankTokenHomophoneOptions { token, options }
                })
                .collect::<Vec<_>>())
        })
    })
    .await
    .map_err(|err| format!("Failed to join enabled homophone query task: {err}"))?
}

/// 懒加载共享词库加载器，并在同一把锁下执行具体操作。
fn with_wordbank_loader<T>(
    app: &AppHandle,
    state: &Arc<Mutex<Option<WordbankLoader>>>,
    op: impl FnOnce(&mut WordbankLoader) -> Result<T>,
) -> Result<T, String> {
    let mut guard = state
        .lock()
        .map_err(|_| "Failed to lock wordbank state".to_string())?;
    if guard.is_none() {
        let path = resolve_wordbank_path(app)
            .map_err(|err| format!("failed to resolve wordbank path: {err:#}"))?;
        *guard = Some(
            WordbankLoader::open(path)
                .map_err(|err| format!("failed to initialize wordbank: {err:#}"))?,
        );
    }

    let loader = guard
        .as_mut()
        .ok_or_else(|| "wordbank loader was not initialized".to_string())?;
    op(loader).map_err(|err| format!("wordbank operation failed: {err:#}"))
}

/// 根据当前应用配置解析词库数据库路径。
fn resolve_wordbank_path(app: &AppHandle) -> Result<PathBuf> {
    let base = app
        .path()
        .app_local_data_dir()
        .context("app_local_data_dir was not available")?;
    Ok(base.join(WORDBANK_FILENAME))
}

fn resolve_wordbank_backup_dir(app: &AppHandle) -> Result<PathBuf> {
    let base = app
        .path()
        .app_local_data_dir()
        .context("app_local_data_dir was not available")?;
    Ok(base.join(WORDBANK_BACKUP_DIRNAME))
}

fn normalize_wordbank_id(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(!value.is_empty(), "wordbank id cannot be empty");
    Ok(value.to_string())
}

fn normalize_bank_name(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(!value.is_empty(), "wordbank name cannot be empty");
    Ok(value.to_string())
}

fn normalize_description(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_affix(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
}

fn sort_wordbank_meta_record(
    left_id: &str,
    left: &WordbankMetaRecord,
    right_id: &str,
    right: &WordbankMetaRecord,
) -> std::cmp::Ordering {
    right
        .is_default
        .cmp(&left.is_default)
        .then_with(|| left.sort_order.cmp(&right.sort_order))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left_id.cmp(right_id))
}

fn sort_wordbank_meta_entry(
    left: &(String, WordbankMetaRecord),
    right: &(String, WordbankMetaRecord),
) -> std::cmp::Ordering {
    sort_wordbank_meta_record(&left.0, &left.1, &right.0, &right.1)
}

fn sort_wordbank_summary(left: &WordbankSummary, right: &WordbankSummary) -> std::cmp::Ordering {
    right
        .is_default
        .cmp(&left.is_default)
        .then_with(|| left.sort_order.cmp(&right.sort_order))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.id.cmp(&right.id))
}

/// 将词条规范化为仅包含中文字符的字符串。
fn normalize_word(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(!value.is_empty(), "word entry cannot be empty");
    ensure!(
        value.chars().all(|ch| ch.to_pinyin().is_some()),
        "word bank only supports Chinese characters"
    );
    Ok(value.to_string())
}

fn normalize_entry_key(value: &str) -> Result<String> {
    let value = value.trim();
    ensure!(!value.is_empty(), "wordbank entry key cannot be empty");
    Ok(value.to_string())
}

fn normalize_entry_group_values(values: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();

    for value in values {
        let value = normalize_word(&value)?;
        ensure!(
            seen.insert(value.clone()),
            "reordered values cannot contain duplicates"
        );
        normalized.push(value);
    }

    ensure!(
        !normalized.is_empty(),
        "reordered values must contain at least one entry"
    );
    Ok(normalized)
}

fn split_wordbank_text(text: &str) -> Result<Vec<String>> {
    let values = text
        .split_whitespace()
        .map(normalize_word)
        .collect::<Result<Vec<_>>>()?;
    ensure!(
        !values.is_empty(),
        "word bank input did not contain any Chinese entries"
    );
    Ok(values)
}

/// 以无声调拼音构造数据库键。
fn build_pinyin_key(value: &str) -> Result<String> {
    let mut syllables = Vec::new();
    for ch in value.chars() {
        let pinyin = ch
            .to_pinyin()
            .ok_or_else(|| anyhow!("character '{ch}' did not have pinyin"))?;
        syllables.push(pinyin.plain().to_string());
    }
    ensure!(
        !syllables.is_empty(),
        "word entry did not produce a pinyin key"
    );
    Ok(syllables.join(" "))
}

fn generate_wordbank_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("wb_{nanos}")
}

fn encode_meta_record(record: &WordbankMetaRecord) -> Result<String> {
    serde_json::to_string(record).context("failed to encode wordbank metadata")
}

fn decode_meta_record(value: &str) -> Result<WordbankMetaRecord> {
    serde_json::from_str(value).context("failed to decode wordbank metadata")
}

fn encode_entry_order_record(record: &WordbankEntryOrderRecord) -> Result<String> {
    serde_json::to_string(record).context("failed to encode wordbank entry order")
}

fn decode_entry_order_record(value: &str) -> Result<WordbankEntryOrderRecord> {
    serde_json::from_str(value).context("failed to decode wordbank entry order")
}

fn ensure_wordbank_exists_in_read(
    txn: &redb::ReadTransaction,
    wordbank_id: &str,
) -> Result<WordbankMetaRecord> {
    let meta_table = txn
        .open_table(WORDBANK_META_TABLE)
        .context("failed to open wordbank metadata table")?;
    let meta = meta_table
        .get(wordbank_id)
        .with_context(|| format!("failed to query wordbank '{wordbank_id}'"))?
        .ok_or_else(|| anyhow!("wordbank '{wordbank_id}' was not found"))?;
    decode_meta_record(meta.value())
}

fn ensure_wordbank_exists_in_write(
    txn: &redb::WriteTransaction,
    wordbank_id: &str,
) -> Result<WordbankMetaRecord> {
    let meta_table = txn
        .open_table(WORDBANK_META_TABLE)
        .context("failed to open wordbank metadata table")?;
    let meta = meta_table
        .get(wordbank_id)
        .with_context(|| format!("failed to query wordbank '{wordbank_id}'"))?
        .ok_or_else(|| anyhow!("wordbank '{wordbank_id}' was not found"))?;
    decode_meta_record(meta.value())
}

fn ensure_wordbank_tables(
    write_txn: &redb::WriteTransaction,
    word_table_name: &'static str,
    homophone_table_name: &'static str,
    order_table_name: &'static str,
) -> Result<()> {
    open_word_table_write(write_txn, word_table_name).context("failed to create word table")?;
    open_homophone_table_write(write_txn, homophone_table_name)
        .context("failed to create homophone table")?;
    open_order_table_write(write_txn, order_table_name).context("failed to create order table")?;
    Ok(())
}

fn word_table_name(wordbank_id: &str) -> String {
    format!("wordbank__{wordbank_id}__words")
}

fn homophone_table_name(wordbank_id: &str) -> String {
    format!("wordbank__{wordbank_id}__homophones")
}

fn order_table_name(wordbank_id: &str) -> String {
    format!("wordbank__{wordbank_id}__order")
}

fn word_table_def(table_name: &str) -> TableDefinition<'_, &str, &str> {
    TableDefinition::new(table_name)
}

fn homophone_table_def(table_name: &str) -> MultimapTableDefinition<'_, &str, &str> {
    MultimapTableDefinition::new(table_name)
}

fn order_table_def(table_name: &str) -> TableDefinition<'_, &str, &str> {
    TableDefinition::new(table_name)
}

fn open_word_table_write<'txn>(
    txn: &'txn redb::WriteTransaction,
    table_name: &'static str,
) -> Result<redb::Table<'txn, &'static str, &'static str>, redb::TableError> {
    txn.open_table(word_table_def(table_name))
}

fn open_homophone_table_write<'txn>(
    txn: &'txn redb::WriteTransaction,
    table_name: &'static str,
) -> Result<redb::MultimapTable<'txn, &'static str, &'static str>, redb::TableError> {
    txn.open_multimap_table(homophone_table_def(table_name))
}

fn open_order_table_write<'txn>(
    txn: &'txn redb::WriteTransaction,
    table_name: &'static str,
) -> Result<redb::Table<'txn, &'static str, &'static str>, redb::TableError> {
    txn.open_table(order_table_def(table_name))
}

fn open_word_table_read<'txn>(
    txn: &'txn redb::ReadTransaction,
    table_name: &'static str,
) -> Result<redb::ReadOnlyTable<&'static str, &'static str>, redb::TableError> {
    txn.open_table(word_table_def(table_name))
}

fn open_homophone_table_read<'txn>(
    txn: &'txn redb::ReadTransaction,
    table_name: &'static str,
) -> Result<redb::ReadOnlyMultimapTable<&'static str, &'static str>, redb::TableError> {
    txn.open_multimap_table(homophone_table_def(table_name))
}

fn open_order_table_read<'txn>(
    txn: &'txn redb::ReadTransaction,
    table_name: &'static str,
) -> Result<redb::ReadOnlyTable<&'static str, &'static str>, redb::TableError> {
    txn.open_table(order_table_def(table_name))
}

fn delete_word_table(
    txn: &redb::WriteTransaction,
    table_name: &'static str,
) -> Result<bool, redb::TableError> {
    txn.delete_table(word_table_def(table_name))
}

fn delete_homophone_table(
    txn: &redb::WriteTransaction,
    table_name: &'static str,
) -> Result<bool, redb::TableError> {
    txn.delete_multimap_table(homophone_table_def(table_name))
}

fn delete_order_table(
    txn: &redb::WriteTransaction,
    table_name: &'static str,
) -> Result<bool, redb::TableError> {
    txn.delete_table(order_table_def(table_name))
}

fn read_order_record<'a>(
    order_table: &'a redb::ReadOnlyTable<&'static str, &'static str>,
    key: &str,
) -> Result<Option<Vec<String>>> {
    let raw = order_table
        .get(key)
        .with_context(|| format!("failed to read order for key '{key}'"))?;
    raw.map(|value: redb::AccessGuard<'_, &'static str>| {
        decode_entry_order_record(value.value()).map(|record| record.values)
    })
    .transpose()
}

fn read_order_record_write(
    order_table: &redb::Table<'_, &str, &str>,
    key: &str,
) -> Result<Option<Vec<String>>> {
    let raw = order_table
        .get(key)
        .with_context(|| format!("failed to read order for key '{key}'"))?;
    raw.map(|value| decode_entry_order_record(value.value()).map(|record| record.values))
        .transpose()
}

fn store_order_record(
    order_table: &mut redb::Table<'_, &str, &str>,
    key: &str,
    values: &[String],
) -> Result<()> {
    let record = WordbankEntryOrderRecord {
        values: values.to_vec(),
    };
    order_table
        .insert(key, encode_entry_order_record(&record)?.as_str())
        .with_context(|| format!("failed to store order for key '{key}'"))?;
    Ok(())
}

fn load_all_values_read(
    word_table: &redb::ReadOnlyTable<&'static str, &'static str>,
    homophone_table: &redb::ReadOnlyMultimapTable<&'static str, &'static str>,
    key: &str,
) -> Result<Vec<String>> {
    let primary_value = match word_table
        .get(key)
        .with_context(|| format!("failed to query key '{key}'"))?
    {
        Some(value) => {
            let value: redb::AccessGuard<'_, &'static str> = value;
            value.value().to_string()
        }
        None => return Ok(Vec::new()),
    };

    let mut values = vec![primary_value.clone()];
    let mut seen = HashSet::from([primary_value]);
    let mut iter = homophone_table
        .get(key)
        .with_context(|| format!("failed to read homophones for key '{key}'"))?;
    while let Some(value) = iter.next() {
        let value: redb::AccessGuard<'_, &'static str> =
            value.context("failed to read homophone value")?;
        let value = value.value().to_string();
        if seen.insert(value.clone()) {
            values.push(value);
        }
    }
    Ok(values)
}

fn load_all_values_write(
    word_table: &redb::Table<'_, &str, &str>,
    homophone_table: &redb::MultimapTable<'_, &str, &str>,
    key: &str,
) -> Result<Vec<String>> {
    let primary_value = match word_table
        .get(key)
        .with_context(|| format!("failed to query key '{key}'"))?
    {
        Some(value) => value.value().to_string(),
        None => return Ok(Vec::new()),
    };

    let mut values = vec![primary_value.clone()];
    let mut seen = HashSet::from([primary_value]);
    let mut iter = homophone_table
        .get(key)
        .with_context(|| format!("failed to read homophones for key '{key}'"))?;
    while let Some(value) = iter.next() {
        let value = value.context("failed to read homophone value")?;
        let value = value.value().to_string();
        if seen.insert(value.clone()) {
            values.push(value);
        }
    }
    Ok(values)
}

fn merge_values_with_order(
    all_values: Vec<String>,
    order_values: Option<Vec<String>>,
) -> Vec<String> {
    let Some(order_values) = order_values else {
        return all_values;
    };

    let all_set = all_values.iter().cloned().collect::<HashSet<_>>();
    let mut merged = Vec::with_capacity(all_values.len());
    let mut seen = HashSet::new();

    for value in order_values {
        if all_set.contains(&value) && seen.insert(value.clone()) {
            merged.push(value);
        }
    }

    for value in all_values {
        if seen.insert(value.clone()) {
            merged.push(value);
        }
    }

    merged
}

fn load_ordered_values_read(
    word_table: &redb::ReadOnlyTable<&'static str, &'static str>,
    homophone_table: &redb::ReadOnlyMultimapTable<&'static str, &'static str>,
    order_table: &redb::ReadOnlyTable<&'static str, &'static str>,
    key: &str,
) -> Result<Vec<String>> {
    let all_values = load_all_values_read(word_table, homophone_table, key)?;
    let order_values = read_order_record(order_table, key)?;
    Ok(merge_values_with_order(all_values, order_values))
}

fn load_ordered_values_write(
    word_table: &redb::Table<'_, &str, &str>,
    homophone_table: &redb::MultimapTable<'_, &str, &str>,
    order_table: &redb::Table<'_, &str, &str>,
    key: &str,
) -> Result<Vec<String>> {
    let all_values = load_all_values_write(word_table, homophone_table, key)?;
    let order_values = read_order_record_write(order_table, key)?;
    Ok(merge_values_with_order(all_values, order_values))
}

/// 插入词条；若拼音键已存在，则追加到同音表中。
fn insert_value(
    word_table: &mut redb::Table<'_, &str, &str>,
    homophone_table: &mut redb::MultimapTable<'_, &str, &str>,
    order_table: &mut redb::Table<'_, &str, &str>,
    key: &str,
    value: &str,
) -> Result<()> {
    let mut values = load_ordered_values_write(word_table, homophone_table, order_table, key)
        .unwrap_or_default();
    if values.iter().any(|existing| existing == value) {
        return Ok(());
    }

    if values.is_empty() {
        word_table
            .insert(key, value)
            .with_context(|| format!("failed to insert value '{value}' under key '{key}'"))?;
        values.push(value.to_string());
    } else {
        homophone_table
            .insert(key, value)
            .with_context(|| format!("failed to append value '{value}' under key '{key}'"))?;
        values.push(value.to_string());
    }
    store_order_record(order_table, key, &values)?;

    Ok(())
}

/// 删除一个词条，并在必要时提升同音表中的备选值为主值。
fn remove_value(
    word_table: &mut redb::Table<'_, &str, &str>,
    homophone_table: &mut redb::MultimapTable<'_, &str, &str>,
    order_table: &mut redb::Table<'_, &str, &str>,
    key: &str,
    value: &str,
) -> Result<()> {
    let primary_value = word_table
        .get(key)
        .with_context(|| format!("failed to query key '{key}'"))?
        .map(|current| current.value().to_string())
        .ok_or_else(|| anyhow!("word entry '{value}' was not found"))?;
    let mut values = load_ordered_values_write(word_table, homophone_table, order_table, key)?;
    let previous_len = values.len();
    values.retain(|existing| existing != value);
    ensure!(
        values.len() != previous_len,
        "word entry '{value}' was not found"
    );

    if primary_value != value {
        let removed = homophone_table
            .remove(key, value)
            .with_context(|| format!("failed to remove homophone value '{value}'"))?;
        ensure!(removed, "word entry '{value}' was not found");
    }

    if values.is_empty() {
        word_table
            .remove(key)
            .with_context(|| format!("failed to remove key '{key}'"))?;
        order_table
            .remove(key)
            .with_context(|| format!("failed to remove order for key '{key}'"))?;
        homophone_table
            .remove_all(key)
            .with_context(|| format!("failed to clear homophones for key '{key}'"))?;
        return Ok(());
    }

    if primary_value == value {
        homophone_table
            .remove_all(key)
            .with_context(|| format!("failed to clear homophones for key '{key}'"))?;
        word_table
            .insert(key, values[0].as_str())
            .with_context(|| {
                format!("failed to promote value '{}' under key '{key}'", values[0])
            })?;

        for alternative in values.iter().skip(1) {
            homophone_table
                .insert(key, alternative.as_str())
                .with_context(|| format!("failed to restore homophone value '{alternative}'"))?;
        }
    }

    store_order_record(order_table, key, &values)?;
    Ok(())
}
