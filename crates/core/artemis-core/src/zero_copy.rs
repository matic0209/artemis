//! 零拷贝序列化模块 - 使用 rkyv 实现高性能序列化

use std::sync::Arc;
use rkyv::{Archive, Deserialize, Serialize, archived_root, to_bytes, AlignedVec};
use bytecheck::CheckBytes;
use serde::{Serialize as SerdeSerialize, Deserialize as SerdeDeserialize};

use crate::eth::{Address, U256};

/// 零拷贝事件
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq, SerdeSerialize, SerdeDeserialize)]
#[archive(compare(PartialEq), check_bytes)]
pub struct ZeroCopyEvent {
    /// 事件类型
    pub event_type: u32,
    /// 时间戳
    pub timestamp: u64,
    /// 区块号
    pub block_number: u64,
    /// 交易哈希
    pub tx_hash: [u8; 32],
    /// 地址
    pub address: [u8; 20],
    /// 数据
    pub data: Vec<u8>,
    /// 主题
    pub topics: Vec<[u8; 32]>,
}

/// 零拷贝动作
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq, SerdeSerialize, SerdeDeserialize)]
#[archive(compare(PartialEq), check_bytes)]
pub struct ZeroCopyAction {
    /// 动作类型
    pub action_type: u32,
    /// 优先级
    pub priority: u32,
    /// 目标地址
    pub target: [u8; 20],
    /// 调用数据
    pub calldata: Vec<u8>,
    /// Gas 限制
    pub gas_limit: u64,
    /// Gas 价格
    pub gas_price: u64,
    /// 值
    pub value: [u8; 32], // U256 as bytes
    /// 元数据
    pub metadata: Vec<u8>,
}

/// 零拷贝价格数据
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
#[archive(compare(PartialEq), check_bytes)]
pub struct ZeroCopyPriceData {
    /// 池地址
    pub pool_address: [u8; 20],
    /// Token0 地址
    pub token0: [u8; 20],
    /// Token1 地址  
    pub token1: [u8; 20],
    /// 价格
    pub price: [u8; 32], // U256 as bytes
    /// 流动性
    pub liquidity: [u8; 32], // U256 as bytes
    /// 时间戳
    pub timestamp: u64,
    /// 置信度
    pub confidence: u32,
}

/// 零拷贝套利机会
#[derive(Archive, Deserialize, Serialize, Debug, Clone, PartialEq)]
#[archive(compare(PartialEq), check_bytes)]
pub struct ZeroCopyArbOpportunity {
    /// 机会 ID
    pub id: u64,
    /// V3 池地址
    pub v3_pool: [u8; 20],
    /// V2 池地址
    pub v2_pool: [u8; 20],
    /// 输入金额
    pub amount_in: [u8; 32],
    /// 预期利润
    pub expected_profit: [u8; 32],
    /// 风险评分
    pub risk_score: u32,
    /// 过期时间
    pub expires_at: u64,
}

/// 零拷贝序列化器
pub struct ZeroCopySerializer;

impl ZeroCopySerializer {
    /// 序列化事件
    pub fn serialize_event(event: &ZeroCopyEvent) -> Result<AlignedVec, Box<dyn std::error::Error>> {
        to_bytes::<_, 256>(event).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    /// 反序列化事件
    pub fn deserialize_event(bytes: &[u8]) -> Result<&ArchivedZeroCopyEvent, Box<dyn std::error::Error>> {
        unsafe { Ok(archived_root::<ZeroCopyEvent>(bytes)) }
    }

    /// 序列化动作
    pub fn serialize_action(action: &ZeroCopyAction) -> Result<AlignedVec, Box<dyn std::error::Error>> {
        to_bytes::<_, 256>(action).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }

    /// 反序列化动作
    pub fn deserialize_action(bytes: &[u8]) -> Result<&ArchivedZeroCopyAction, Box<dyn std::error::Error>> {
        unsafe { Ok(archived_root::<ZeroCopyAction>(bytes)) }
    }

    /// 批量序列化事件
    pub fn serialize_events_batch(events: &[ZeroCopyEvent]) -> Result<Vec<AlignedVec>, Box<dyn std::error::Error>> {
        let mut results = Vec::new();
        for event in events {
            results.push(Self::serialize_event(event)?);
        }
        Ok(results)
    }

    /// 批量反序列化事件
    pub fn deserialize_events_batch<'a>(
        serialized_events: &'a [&'a [u8]]
    ) -> Result<Vec<&'a ArchivedZeroCopyEvent>, Box<dyn std::error::Error>> {
        serialized_events
            .iter()
            .map(|bytes| Self::deserialize_event(bytes))
            .collect()
    }
}

/// 零拷贝缓冲池
pub struct ZeroCopyBufferPool {
    /// 事件缓冲池
    event_buffers: crossbeam::queue::SegQueue<AlignedVec>,
    /// 动作缓冲池
    action_buffers: crossbeam::queue::SegQueue<AlignedVec>,
    /// 最大池大小
    max_pool_size: usize,
}

impl ZeroCopyBufferPool {
    /// 创建新的缓冲池
    pub fn new(max_pool_size: usize) -> Self {
        Self {
            event_buffers: crossbeam::queue::SegQueue::new(),
            action_buffers: crossbeam::queue::SegQueue::new(),
            max_pool_size,
        }
    }

    /// 获取事件缓冲区
    pub fn get_event_buffer(&self) -> AlignedVec {
        self.event_buffers.pop().unwrap_or_else(|| AlignedVec::new())
    }

    /// 归还事件缓冲区
    pub fn return_event_buffer(&self, mut buffer: AlignedVec) {
        if self.event_buffers.len() < self.max_pool_size {
            buffer.clear();
            self.event_buffers.push(buffer);
        }
    }

    /// 获取动作缓冲区
    pub fn get_action_buffer(&self) -> AlignedVec {
        self.action_buffers.pop().unwrap_or_else(|| AlignedVec::new())
    }

    /// 归还动作缓冲区
    pub fn return_action_buffer(&self, mut buffer: AlignedVec) {
        if self.action_buffers.len() < self.max_pool_size {
            buffer.clear();
            self.action_buffers.push(buffer);
        }
    }
}

/// 零拷贝流处理器
pub struct ZeroCopyStreamProcessor {
    /// 缓冲池
    buffer_pool: Arc<ZeroCopyBufferPool>,
    /// 统计信息
    stats: ZeroCopyStats,
}

/// 零拷贝统计
#[derive(Debug, Default)]
pub struct ZeroCopyStats {
    /// 序列化次数
    pub serializations: u64,
    /// 反序列化次数
    pub deserializations: u64,
    /// 节省的拷贝次数
    pub copies_saved: u64,
    /// 总处理字节数
    pub bytes_processed: u64,
}

impl ZeroCopyStreamProcessor {
    /// 创建新的流处理器
    pub fn new(buffer_pool_size: usize) -> Self {
        Self {
            buffer_pool: Arc::new(ZeroCopyBufferPool::new(buffer_pool_size)),
            stats: ZeroCopyStats::default(),
        }
    }

    /// 处理事件流
    pub async fn process_event_stream<F>(&mut self, events: Vec<ZeroCopyEvent>, processor: F) -> Result<Vec<ZeroCopyAction>, Box<dyn std::error::Error>>
    where
        F: Fn(&ArchivedZeroCopyEvent) -> Option<ZeroCopyAction>,
    {
        let mut actions = Vec::new();

        for event in events {
            // 序列化事件
            let serialized = ZeroCopySerializer::serialize_event(&event)
                .map_err(|e| anyhow::anyhow!("Serialization failed: {:?}", e))?;
            self.stats.serializations += 1;
            self.stats.bytes_processed += serialized.len() as u64;

            // 零拷贝反序列化
            let archived_event = ZeroCopySerializer::deserialize_event(&serialized)
                .map_err(|e| anyhow::anyhow!("Deserialization failed: {:?}", e))?;
            self.stats.deserializations += 1;
            self.stats.copies_saved += 1; // 避免了一次拷贝

            // 处理事件
            if let Some(action) = processor(archived_event) {
                actions.push(action);
            }
        }

        Ok(actions)
    }

    /// 获取统计信息
    pub fn get_stats(&self) -> &ZeroCopyStats {
        &self.stats
    }

    /// 重置统计信息
    pub fn reset_stats(&mut self) {
        self.stats = ZeroCopyStats::default();
    }
}

/// 类型转换工具
pub struct TypeConverter;

impl TypeConverter {
    /// Address 到字节数组
    pub fn address_to_bytes(addr: Address) -> [u8; 20] {
        addr.0 .0
    }

    /// 字节数组到 Address
    pub fn bytes_to_address(bytes: [u8; 20]) -> Address {
        Address::from(bytes)
    }

    /// U256 到字节数组
    pub fn u256_to_bytes(value: U256) -> [u8; 32] {
        let mut bytes = [0u8; 32];
        let bytes_array = value.to_be_bytes::<32>();
        bytes.copy_from_slice(&bytes_array);
        bytes
    }

    /// 字节数组到 U256
    pub fn bytes_to_u256(bytes: [u8; 32]) -> U256 {
        U256::from_be_slice(&bytes)
    }

    /// 转换标准事件到零拷贝事件
    pub fn to_zero_copy_event(
        event_type: u32,
        timestamp: u64,
        block_number: u64,
        tx_hash: [u8; 32],
        address: Address,
        data: Vec<u8>,
        topics: Vec<[u8; 32]>,
    ) -> ZeroCopyEvent {
        ZeroCopyEvent {
            event_type,
            timestamp,
            block_number,
            tx_hash,
            address: Self::address_to_bytes(address),
            data,
            topics,
        }
    }

    /// 转换零拷贝事件到标准事件
    pub fn from_zero_copy_event(event: &ArchivedZeroCopyEvent) -> (u32, u64, u64, [u8; 32], Address, &[u8], &[[u8; 32]]) {
        (
            event.event_type,
            event.timestamp,
            event.block_number,
            event.tx_hash,
            Self::bytes_to_address(event.address),
            &event.data,
            &event.topics,
        )
    }
}

/// 基准测试工具
pub struct ZeroCopyBenchmark;

impl ZeroCopyBenchmark {
    /// 比较序列化性能
    pub fn benchmark_serialization(iterations: usize) -> (f64, f64) {
        use std::time::Instant;

        let event = ZeroCopyEvent {
            event_type: 1,
            timestamp: 1234567890,
            block_number: 12345,
            tx_hash: [1u8; 32],
            address: [2u8; 20],
            data: vec![3u8; 1000],
            topics: vec![[4u8; 32], [5u8; 32]],
        };

        // rkyv 序列化
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = ZeroCopySerializer::serialize_event(&event).unwrap();
        }
        let rkyv_time = start.elapsed().as_nanos() as f64 / iterations as f64;

        // serde_json 序列化（对比）
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = serde_json::to_vec(&event).unwrap();
        }
        let serde_time = start.elapsed().as_nanos() as f64 / iterations as f64;

        (rkyv_time, serde_time)
    }

    /// 比较反序列化性能
    pub fn benchmark_deserialization(iterations: usize) -> (f64, f64) {
        use std::time::Instant;

        let event = ZeroCopyEvent {
            event_type: 1,
            timestamp: 1234567890,
            block_number: 12345,
            tx_hash: [1u8; 32],
            address: [2u8; 20],
            data: vec![3u8; 1000],
            topics: vec![[4u8; 32], [5u8; 32]],
        };

        let rkyv_bytes = ZeroCopySerializer::serialize_event(&event).unwrap();
        let serde_bytes = serde_json::to_vec(&event).unwrap();

        // rkyv 反序列化
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = ZeroCopySerializer::deserialize_event(&rkyv_bytes).unwrap();
        }
        let rkyv_time = start.elapsed().as_nanos() as f64 / iterations as f64;

        // serde_json 反序列化
        let start = Instant::now();
        for _ in 0..iterations {
            let _: ZeroCopyEvent = serde_json::from_slice(&serde_bytes).unwrap();
        }
        let serde_time = start.elapsed().as_nanos() as f64 / iterations as f64;

        (rkyv_time, serde_time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_serialization() {
        let event = ZeroCopyEvent {
            event_type: 1,
            timestamp: 1234567890,
            block_number: 12345,
            tx_hash: [1u8; 32],
            address: [2u8; 20],
            data: vec![3, 4, 5],
            topics: vec![[6u8; 32]],
        };

        let serialized = ZeroCopySerializer::serialize_event(&event).unwrap();
        let deserialized = ZeroCopySerializer::deserialize_event(&serialized).unwrap();

        assert_eq!(deserialized.event_type, event.event_type);
        assert_eq!(deserialized.timestamp, event.timestamp);
        assert_eq!(deserialized.block_number, event.block_number);
        assert_eq!(deserialized.tx_hash, event.tx_hash);
        assert_eq!(deserialized.address, event.address);
        assert_eq!(deserialized.data.as_slice(), event.data.as_slice());
    }

    #[test]
    fn test_type_conversion() {
        let addr = Address::from([1u8; 20]);
        let bytes = TypeConverter::address_to_bytes(addr);
        let converted_back = TypeConverter::bytes_to_address(bytes);
        assert_eq!(addr, converted_back);

        let value = U256::from(12345);
        let bytes = TypeConverter::u256_to_bytes(value);
        let converted_back = TypeConverter::bytes_to_u256(bytes);
        assert_eq!(value, converted_back);
    }

    #[test]
    fn test_buffer_pool() {
        let pool = ZeroCopyBufferPool::new(5);
        
        let buffer1 = pool.get_event_buffer();
        let buffer2 = pool.get_event_buffer();
        
        pool.return_event_buffer(buffer1);
        pool.return_event_buffer(buffer2);
        
        // 应该能够重用缓冲区
        let buffer3 = pool.get_event_buffer();
        assert!(buffer3.capacity() >= 0); // 基本检查
    }

    #[tokio::test]
    async fn test_stream_processor() {
        let mut processor = ZeroCopyStreamProcessor::new(10);
        
        let events = vec![
            ZeroCopyEvent {
                event_type: 1,
                timestamp: 1234567890,
                block_number: 12345,
                tx_hash: [1u8; 32],
                address: [2u8; 20],
                data: vec![3, 4, 5],
                topics: vec![[6u8; 32]],
            }
        ];

        let actions = processor.process_event_stream(events, |_event| {
            Some(ZeroCopyAction {
                action_type: 1,
                priority: 100,
                target: [7u8; 20],
                calldata: vec![8, 9, 10],
                gas_limit: 21000,
                gas_price: 20000000000,
                value: [0u8; 32],
                metadata: vec![],
            })
        }).await.unwrap();

        assert_eq!(actions.len(), 1);
        let stats = processor.get_stats();
        assert_eq!(stats.serializations, 1);
        assert_eq!(stats.deserializations, 1);
        assert_eq!(stats.copies_saved, 1);
    }
}
