//! 内存池管理器 - 减少内存分配开销

use std::sync::Arc;
use parking_lot::Mutex;
use crossbeam::queue::SegQueue;

/// 内存池管理器
pub struct MemoryPool<T> {
    pool: SegQueue<Box<T>>,
    max_size: usize,
    current_size: Arc<Mutex<usize>>,
}

impl<T> MemoryPool<T> {
    /// 创建新的内存池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: SegQueue::new(),
            max_size,
            current_size: Arc::new(Mutex::new(0)),
        }
    }

    /// 从池中获取对象
    pub fn get(&self) -> Box<T> 
    where 
        T: Default,
    {
        if let Some(item) = self.pool.pop() {
            item
        } else {
            Box::new(T::default())
        }
    }

    /// 将对象返回到池中
    pub fn put(&self, mut item: Box<T>) 
    where 
        T: Default,
    {
        let current_size = {
            let size_guard = self.current_size.lock();
            *size_guard
        };

        if current_size < self.max_size {
            // 重置对象状态
            *item = T::default();
            self.pool.push(item);
            
            let mut size_guard = self.current_size.lock();
            *size_guard += 1;
        }
        // 如果池已满，对象将被丢弃
    }

    /// 获取当前池大小
    pub fn len(&self) -> usize {
        let size_guard = self.current_size.lock();
        *size_guard
    }

    /// 检查池是否为空
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 清空池
    pub fn clear(&self) {
        while self.pool.pop().is_some() {}
        let mut size_guard = self.current_size.lock();
        *size_guard = 0;
    }
}

/// 池化的向量
pub type PooledVec<T> = Box<Vec<T>>;

/// 向量池管理器
pub struct VecPool<T> {
    pool: MemoryPool<Vec<T>>,
}

impl<T> VecPool<T> {
    /// 创建新的向量池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: MemoryPool::new(max_size),
        }
    }

    /// 获取池化向量
    pub fn get(&self) -> PooledVec<T> {
        let mut vec = self.pool.get();
        vec.clear(); // 确保向量为空
        vec
    }

    /// 返回向量到池
    pub fn put(&self, vec: PooledVec<T>) {
        self.pool.put(vec);
    }
}

/// 池化的哈希映射
use std::collections::HashMap;
use std::hash::Hash;

pub type PooledHashMap<K, V> = Box<HashMap<K, V>>;

/// 哈希映射池管理器
pub struct HashMapPool<K, V> 
where 
    K: Eq + Hash,
{
    pool: SegQueue<PooledHashMap<K, V>>,
    max_size: usize,
    current_size: Arc<Mutex<usize>>,
}

impl<K, V> HashMapPool<K, V> 
where 
    K: Eq + Hash,
{
    /// 创建新的哈希映射池
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: SegQueue::new(),
            max_size,
            current_size: Arc::new(Mutex::new(0)),
        }
    }

    /// 获取池化哈希映射
    pub fn get(&self) -> PooledHashMap<K, V> {
        if let Some(mut map) = self.pool.pop() {
            map.clear(); // 清空内容但保留容量
            map
        } else {
            Box::new(HashMap::new())
        }
    }

    /// 返回哈希映射到池
    pub fn put(&self, map: PooledHashMap<K, V>) {
        let current_size = {
            let size_guard = self.current_size.lock();
            *size_guard
        };

        if current_size < self.max_size {
            self.pool.push(map);
            let mut size_guard = self.current_size.lock();
            *size_guard += 1;
        }
    }
}

/// 全局内存池管理器
pub struct GlobalMemoryPools {
    /// 事件向量池
    pub event_vec_pool: VecPool<crate::types::Events>,
    /// 动作向量池  
    pub action_vec_pool: VecPool<crate::types::Actions>,
    /// 字符串池
    pub string_pool: MemoryPool<String>,
    /// 字节向量池
    pub bytes_pool: VecPool<u8>,
}

impl GlobalMemoryPools {
    /// 创建全局内存池
    pub fn new() -> Self {
        Self {
            event_vec_pool: VecPool::new(100),
            action_vec_pool: VecPool::new(100),
            string_pool: MemoryPool::new(200),
            bytes_pool: VecPool::new(500),
        }
    }

    /// 获取单例实例
    pub fn instance() -> &'static Self {
        static INSTANCE: std::sync::OnceLock<GlobalMemoryPools> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| Self::new())
    }
}

impl Default for GlobalMemoryPools {
    fn default() -> Self {
        Self::new()
    }
}

/// 便捷宏用于池化分配
#[macro_export]
macro_rules! pooled_vec {
    ($type:ty) => {
        $crate::memory_pool::GlobalMemoryPools::instance().event_vec_pool.get()
    };
}

#[macro_export]
macro_rules! return_to_pool {
    ($pool:expr, $item:expr) => {
        $pool.put($item);
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq)]
    struct TestStruct {
        value: i32,
    }

    #[test]
    fn test_memory_pool_basic_operations() {
        let pool = MemoryPool::<TestStruct>::new(5);
        
        // 获取对象（应该是新创建的）
        let item1 = pool.get();
        assert_eq!(item1.value, 0);
        
        // 修改对象
        let mut item1 = item1;
        item1.value = 42;
        
        // 返回到池
        pool.put(item1);
        assert_eq!(pool.len(), 1);
        
        // 再次获取（应该是重置后的对象）
        let item2 = pool.get();
        assert_eq!(item2.value, 0); // 应该被重置
    }

    #[test]
    fn test_vec_pool() {
        let pool = VecPool::<i32>::new(3);
        
        let mut vec1 = pool.get();
        vec1.push(1);
        vec1.push(2);
        vec1.push(3);
        
        pool.put(vec1);
        
        // 获取应该是清空的向量
        let vec2 = pool.get();
        assert!(vec2.is_empty());
    }

    #[test]
    fn test_hashmap_pool() {
        let pool = HashMapPool::<String, i32>::new(3);
        
        let mut map1 = pool.get();
        map1.insert("key1".to_string(), 1);
        map1.insert("key2".to_string(), 2);
        
        pool.put(map1);
        
        // 获取应该是清空的映射
        let map2 = pool.get();
        assert!(map2.is_empty());
    }

    #[test]
    fn test_pool_size_limit() {
        let pool = MemoryPool::<TestStruct>::new(2);
        
        let item1 = pool.get();
        let item2 = pool.get();
        let item3 = pool.get();
        
        // 返回3个对象，但池最大只能容纳2个
        pool.put(item1);
        pool.put(item2);
        pool.put(item3);
        
        assert_eq!(pool.len(), 2); // 应该只有2个
    }
}
