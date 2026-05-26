//! CRDT (Conflict-free Replicated Data Type) 实时协作
//! 
//! 支持多Agent同时编辑代码，无冲突合并

use super::SwarmError;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// CRDT文档
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrdtDoc {
    pub id: String,
    pub content: CollaborativeText,
    pub version: u64,
    pub agents: Vec<String>, // 参与协作的Agent
}

impl CrdtDoc {
    pub fn new(id: String) -> Self {
        Self {
            id,
            content: CollaborativeText::new(),
            version: 0,
            agents: Vec::new(),
        }
    }
    
    pub fn join(&mut self, agent_id: &str) {
        if !self.agents.contains(&agent_id.to_string()) {
            self.agents.push(agent_id.to_string());
        }
    }
    
    pub fn leave(&mut self, agent_id: &str) {
        self.agents.retain(|a| a != agent_id);
    }
    
    pub fn apply_change(&mut self, change: TextChange) -> Result<(), SwarmError> {
        self.content.apply(change)?;
        self.version += 1;
        Ok(())
    }
    
    pub fn get_text(&self) -> String {
        self.content.to_string()
    }
    
    pub fn merge(&mut self, other: &CrdtDoc) -> Result<(), SwarmError> {
        self.content.merge(&other.content)?;
        self.version = self.version.max(other.version);
        
        for agent in &other.agents {
            if !self.agents.contains(agent) {
                self.agents.push(agent.clone());
            }
        }
        
        Ok(())
    }
}

/// 协作文本 (基于RGA - Replicated Growable Array)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CollaborativeText {
    /// 字符序列，每个字符带有唯一ID和因果信息
    chars: Vec<CharElement>,
    /// 已删除字符的ID集合 (墓碑)
    tombstones: std::collections::HashSet<CharId>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
struct CharId {
    agent_id: String,
    seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CharElement {
    id: CharId,
    value: char,
    /// 左邻居ID (用于排序)
    left: Option<CharId>,
    /// 是否已删除
    deleted: bool,
}

impl CollaborativeText {
    pub fn new() -> Self {
        Self {
            chars: Vec::new(),
            tombstones: std::collections::HashSet::new(),
        }
    }
    
    /// 在指定位置插入文本
    pub fn insert(&mut self, pos: usize, text: &str, agent_id: &str, seq: u64) -> Vec<TextChange> {
        let mut changes = Vec::new();
        let mut current_pos = pos;
        
        for (i, ch) in text.chars().enumerate() {
            let left_id = if current_pos == 0 {
                None
            } else {
                self.chars.get(current_pos - 1).map(|c| c.id.clone())
            };
            
            let char_id = CharId {
                agent_id: agent_id.to_string(),
                seq: seq + i as u64,
            };
            
            let element = CharElement {
                id: char_id.clone(),
                value: ch,
                left: left_id,
                deleted: false,
            };
            
            // 找到正确的插入位置
            let insert_pos = self.find_insert_position(&element);
            self.chars.insert(insert_pos, element);
            
            changes.push(TextChange::Insert {
                pos: current_pos,
                text: ch.to_string(),
                char_id,
            });
            
            current_pos = insert_pos + 1;
        }
        
        changes
    }
    
    /// 删除指定范围的文本
    pub fn delete(&mut self, start: usize, end: usize) -> Vec<TextChange> {
        let mut changes = Vec::new();
        
        for i in start..end.min(self.chars.len()) {
            if let Some(element) = self.chars.get_mut(i) {
                if !element.deleted {
                    element.deleted = true;
                    self.tombstones.insert(element.id.clone());
                    
                    changes.push(TextChange::Delete {
                        pos: i,
                        len: 1,
                        char_id: element.id.clone(),
                    });
                }
            }
        }
        
        changes
    }
    
    /// 应用远程变更
    pub fn apply(&mut self, change: TextChange) -> Result<(), SwarmError> {
        match change {
            TextChange::Insert { pos, text, char_id } => {
                // 检查是否已存在
                if self.chars.iter().any(|c| c.id == char_id) {
                    return Ok(()); // 已存在，幂等
                }
                
                let left = if pos == 0 {
                    None
                } else {
                    self.chars.get(pos.saturating_sub(1)).map(|c| c.id.clone())
                };
                
                let element = CharElement {
                    id: char_id,
                    value: text.chars().next().unwrap_or(' '),
                    left,
                    deleted: false,
                };
                
                let insert_pos = self.find_insert_position(&element);
                self.chars.insert(insert_pos, element);
            }
            TextChange::Delete { pos: _, len: _, char_id } => {
                if let Some(element) = self.chars.iter_mut().find(|c| c.id == char_id) {
                    element.deleted = true;
                    self.tombstones.insert(char_id);
                }
            }
            TextChange::Retain { len } => {
                // 无操作，仅用于保持位置
                let _ = len;
            }
        }
        
        Ok(())
    }
    
    /// 合并另一个协作文本
    pub fn merge(&mut self, other: &CollaborativeText) -> Result<(), SwarmError> {
        // 合并字符
        for other_char in &other.chars {
            if !self.chars.iter().any(|c| c.id == other_char.id) {
                let mut element = other_char.clone();
                let pos = self.find_insert_position(&element);
                self.chars.insert(pos, element);
            }
        }
        
        // 合并删除标记
        for tombstone in &other.tombstones {
            if let Some(element) = self.chars.iter_mut().find(|c| c.id == *tombstone) {
                element.deleted = true;
                self.tombstones.insert(tombstone.clone());
            }
        }
        
        Ok(())
    }
    
    /// 查找插入位置 (基于RGA排序)
    fn find_insert_position(&self, element: &CharElement) -> usize {
        if self.chars.is_empty() {
            return 0;
        }
        
        // 简单的线性查找，实际可用二分优化
        for (i, char) in self.chars.iter().enumerate() {
            if self.compare_position(element, char) == std::cmp::Ordering::Less {
                return i;
            }
        }
        
        self.chars.len()
    }
    
    /// 比较两个字符的位置 (RGA排序)
    fn compare_position(&self, a: &CharElement, b: &CharElement) -> std::cmp::Ordering {
        // 首先比较左邻居
        match (&a.left, &b.left) {
            (None, None) => {
                // 都是开头，按agent_id和seq排序
                a.id.agent_id.cmp(&b.id.agent_id)
                    .then_with(|| a.id.seq.cmp(&b.id.seq))
            }
            (None, Some(_)) => std::cmp::Ordering::Less, // a在开头
            (Some(_), None) => std::cmp::Ordering::Greater, // b在开头
            (Some(left_a), Some(left_b)) => {
                if left_a == left_b {
                    // 相同左邻居，按agent_id和seq排序
                    a.id.agent_id.cmp(&b.id.agent_id)
                        .then_with(|| a.id.seq.cmp(&b.id.seq))
                } else {
                    // 递归比较左邻居
                    if let Some(char_a) = self.chars.iter().find(|c| c.id == *left_a) {
                        if let Some(char_b) = self.chars.iter().find(|c| c.id == *left_b) {
                            return self.compare_position(char_a, char_b);
                        }
                    }
                    std::cmp::Ordering::Equal
                }
            }
        }
    }
    
    /// 获取可见文本 (排除已删除)
    pub fn visible_text(&self) -> String {
        self.chars
            .iter()
            .filter(|c| !c.deleted)
            .map(|c| c.value)
            .collect()
    }
    
    /// 获取完整文本 (包含已删除标记)
    pub fn full_text(&self) -> String {
        self.chars.iter().map(|c| {
            if c.deleted {
                format!("[{}]", c.value)
            } else {
                c.value.to_string()
            }
        }).collect()
    }
    
    /// 计算两个文本的差异
    pub fn diff(&self, other: &CollaborativeText) -> Vec<TextChange> {
        let mut changes = Vec::new();
        let text1 = self.visible_text();
        let text2 = other.visible_text();
        
        // 简单的行级diff，实际可用 Myers算法
        if text1 != text2 {
            changes.push(TextChange::Insert {
                pos: text1.len(),
                text: text2,
                char_id: CharId { agent_id: "diff".to_string(), seq: 0 },
            });
        }
        
        changes
    }
}

impl ToString for CollaborativeText {
    fn to_string(&self) -> String {
        self.visible_text()
    }
}

/// 文本变更操作
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TextChange {
    Insert {
        pos: usize,
        text: String,
        char_id: CharId,
    },
    Delete {
        pos: usize,
        len: usize,
        char_id: CharId,
    },
    Retain {
        len: usize,
    },
}

impl TextChange {
    /// 反转变更 (用于撤销)
    pub fn invert(&self) -> TextChange {
        match self.clone() {
            TextChange::Insert { pos, text, .. } => TextChange::Delete {
                pos,
                len: text.len(),
                char_id: CharId { agent_id: "invert".to_string(), seq: 0 },
            },
            TextChange::Delete { pos, len, .. } => TextChange::Insert {
                pos,
                text: "?".repeat(len), // 简化处理
                char_id: CharId { agent_id: "invert".to_string(), seq: 0 },
            },
            TextChange::Retain { len } => TextChange::Retain { len },
        }
    }
    
    /// 变换变更以适应并发编辑 (OT算法)
    pub fn transform(&self, other: &TextChange) -> TextChange {
        // 简化实现，实际OT更复杂
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collaborative_text_insert() {
        let mut text = CollaborativeText::new();
        text.insert(0, "Hello", "agent1", 0);
        
        assert_eq!(text.visible_text(), "Hello");
    }

    #[test]
    fn test_collaborative_text_delete() {
        let mut text = CollaborativeText::new();
        text.insert(0, "Hello World", "agent1", 0);
        text.delete(5, 6); // 删除 " World"
        
        assert_eq!(text.visible_text(), "Hello");
    }

    #[test]
    fn test_concurrent_insert() {
        let mut text1 = CollaborativeText::new();
        let mut text2 = CollaborativeText::new();
        
        // Agent1 插入 "Hello"
        text1.insert(0, "Hello", "agent1", 0);
        
        // Agent2 并发插入 "World"
        text2.insert(0, "World", "agent2", 0);
        
        // 合并
        text1.merge(&text2).unwrap();
        
        // 结果应该包含两者
        let result = text1.visible_text();
        assert!(result.contains("Hello"));
        assert!(result.contains("World"));
    }

    #[test]
    fn test_crdt_doc() {
        let mut doc = CrdtDoc::new("doc1".to_string());
        doc.join("agent1");
        doc.join("agent2");
        
        let change = TextChange::Insert {
            pos: 0,
            text: "H".to_string(),
            char_id: CharId { agent_id: "agent1".to_string(), seq: 0 },
        };
        
        doc.apply_change(change).unwrap();
        assert_eq!(doc.get_text(), "H");
        assert_eq!(doc.version, 1);
    }
}
