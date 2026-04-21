use anyhow::{ensure, Result};
use jieba_rs::Jieba;

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
