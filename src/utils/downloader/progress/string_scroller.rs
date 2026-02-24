//! 字符串滚动器模块
//! 用于在进度条中滚动显示长文件名

use std::cmp::min;

/// 字符串滚动器
pub struct StringScroller {
    loop_content: String,
    display_width: usize,
    should_scroll: bool,
    char_boundaries: Vec<usize>,
    offset_max: usize,
}

impl StringScroller {
    /// 分隔符空格数
    const SEPARATOR_SPACES: usize = 5;

    /// 创建新的字符串滚动器
    ///
    /// # Arguments
    ///
    /// * `content` - 要显示的字符串
    /// * `display_width` - 显示宽度（字符数）
    ///
    /// # Returns
    ///
    /// * `StringScroller` - 滚动器实例
    pub fn new(content: &str, display_width: usize) -> Self {
        let content_chars_num = content.chars().count();

        // 不需要滚动
        if content_chars_num <= display_width {
            let loop_content = format!("{:<width$}", content, width = display_width);
            Self {
                loop_content,
                display_width,
                should_scroll: false,
                char_boundaries: Vec::new(),
                offset_max: 0,
            }
        } else {
            // 需要滚动
            let mut loop_content =
                String::with_capacity(content.len() * 2 + Self::SEPARATOR_SPACES);
            loop_content.push_str(content);
            loop_content.push_str(&" ".repeat(Self::SEPARATOR_SPACES));
            loop_content.push_str(content);

            let offset_max = content_chars_num + Self::SEPARATOR_SPACES;
            let char_boundaries: Vec<usize> = loop_content.char_indices().map(|(i, _)| i).collect();

            Self {
                loop_content,
                display_width,
                should_scroll: true,
                char_boundaries,
                offset_max,
            }
        }
    }

    /// 获取当前应显示的字符串片段
    ///
    /// # Arguments
    ///
    /// * `offset` - 当前偏移量（字符索引）
    ///
    /// # Returns
    ///
    /// * `&str` - 要显示的字符串片段
    pub fn display(&self, offset: usize) -> &str {
        if !self.should_scroll {
            return &self.loop_content;
        }

        let char_start = offset % self.offset_max;
        let byte_start = self.char_boundaries[char_start];
        let byte_end = self.char_boundaries[min(
            char_start + self.display_width,
            self.char_boundaries.len() - 1,
        )];

        &self.loop_content[byte_start..byte_end]
    }

    /// 是否需要滚动
    pub fn should_scroll(&self) -> bool {
        self.should_scroll
    }

    /// 获取最大偏移量
    pub fn max_offset(&self) -> usize {
        self.offset_max
    }
}
