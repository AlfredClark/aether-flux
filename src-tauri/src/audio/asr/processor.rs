use std::collections::HashMap;

use anyhow::{ensure, Result};
use jieba_rs::Jieba;
use pinyin::ToPinyin;

/// 基于 jieba-rs 的中文分词器封装。
pub struct JiebaDecomposer {
    jieba: Jieba,
}

impl JiebaDecomposer {
    /// 创建一个可复用的 Jieba 分词器实例。
    pub fn new(words: impl IntoIterator<Item = String>) -> Self {
        let mut jieba = Jieba::new();
        for word in words {
            jieba.add_word(word.as_str(), None, None);
        }
        Self { jieba }
    }

    /// 对输入文本执行精确模式分词，并过滤纯空白片段。
    pub fn decompose(&self, text: &str) -> Result<Vec<String>> {
        ensure!(!text.trim().is_empty(), "cannot decompose empty text");

        Ok(self
            .jieba
            .cut(text, false)
            .into_iter()
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(ToOwned::to_owned)
            .collect())
    }
}

/// 基于词库拼音键的 ASR 文本拟合器。
pub struct WordbankFitter {
    preferred_words_by_len: HashMap<usize, HashMap<String, String>>,
    max_word_len: usize,
}

pub struct WordbankFitterReplacement {
    pub key: String,
    pub value: String,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
}

impl WordbankFitter {
    /// 创建一个可复用的词库拟合器实例。
    pub fn new(entries: impl IntoIterator<Item = WordbankFitterReplacement>) -> Self {
        let mut preferred_words_by_len: HashMap<usize, HashMap<String, String>> = HashMap::new();
        let mut max_word_len = 0usize;

        for entry in entries {
            let word_len = entry.value.chars().count();
            if word_len == 0 {
                continue;
            }
            let replacement = format!(
                "{}{}{}",
                entry.prefix.as_deref().unwrap_or_default(),
                entry.value,
                entry.suffix.as_deref().unwrap_or_default()
            );
            max_word_len = max_word_len.max(word_len);
            preferred_words_by_len
                .entry(word_len)
                .or_default()
                .entry(entry.key)
                .or_insert(replacement);
        }

        Self {
            preferred_words_by_len,
            max_word_len,
        }
    }

    /// 使用最长匹配优先的策略，按词库拼音键对识别文本做预拟合。
    pub fn fit(&self, text: &str) -> Result<String> {
        ensure!(!text.trim().is_empty(), "cannot fit empty text");

        if self.max_word_len == 0 {
            return Ok(text.to_string());
        }

        let chars = text.chars().collect::<Vec<_>>();
        let mut output = String::with_capacity(text.len());
        let mut index = 0usize;

        while index < chars.len() {
            let max_len = self.max_word_len.min(chars.len() - index);
            let mut syllables = Vec::with_capacity(max_len);
            let mut best_match: Option<(usize, &str)> = None;

            for len in 1..=max_len {
                let ch = chars[index + len - 1];
                let Some(pinyin) = ch.to_pinyin() else {
                    break;
                };
                syllables.push(pinyin.plain().to_string());

                let Some(words_by_key) = self.preferred_words_by_len.get(&len) else {
                    continue;
                };
                let key = syllables.join(" ");
                if let Some(replacement) = words_by_key.get(key.as_str()) {
                    best_match = Some((len, replacement.as_str()));
                }
            }

            if let Some((matched_len, replacement)) = best_match {
                output.push_str(replacement);
                index += matched_len;
            } else {
                output.push(chars[index]);
                index += 1;
            }
        }

        Ok(output)
    }
}
