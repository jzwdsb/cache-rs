use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Debug;
use std::rc::Rc;
use std::hash::Hash;


pub trait KeyBound: Hash+Eq+Copy+Debug {}
pub trait ValueBound: Copy+Eq+Debug {}

// impl bound for built-in types
macro_rules! impl_bound {
    ($($t:ty),*) => {
        $(
            impl KeyBound for $t {}
            impl ValueBound for $t {}
        )*
    };
}

impl_bound!{u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize, char}


pub trait Cache<K:KeyBound,V:ValueBound> {
    fn get(&mut self, key: &K) -> Option<Rc<V>>;
    fn take(&mut self, key: &K) -> Option<V>;
    fn put(&mut self, key: K, value: V);
    fn remove(&mut self, key: &K);
    fn clear(&mut self);
}

/*
simple cache with no size limit
wrapper around a hashmap
 */
struct SimpleCache<K:KeyBound, V:ValueBound> {
    map: HashMap<K, Rc<V>>,
}


impl<Key:KeyBound, Value:ValueBound> Cache<Key, Value> for SimpleCache<Key, Value> {
    fn get(&mut self, key: &Key) -> Option<Rc<Value>> {
        if self.map.contains_key(key) {
            return Some(self.map.get(key).unwrap().clone())
        }
        None
    }

    // take the ownership of the value and remove it from the cache
    fn take(&mut self, key: &Key) -> Option<Value> {
        if self.map.contains_key(key) {
            return Some(Rc::try_unwrap(self.map.remove(key).unwrap()).unwrap());
        }
        None
    }

    fn put(&mut self, key: Key, value: Value) {
        self.map.insert(key, Rc::new(value));
    }

    // remove the value from the cache
    fn remove(&mut self, key: &Key) {
        _ = self.take(key)
    }

    fn clear(&mut self) {
        self.map.clear();
    }
}

impl<Key: KeyBound, Value:ValueBound> SimpleCache<Key, Value> {
    #[allow(dead_code)]
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }
}

/*
a lru cache with a size limit
use hash map and double linked list
put the most recently used item at the front of the list
when the cache is full, remove the least recently used item from the back of the list
 */
#[derive(Debug)]
struct ListNode<Key:KeyBound, Value:ValueBound> {
    key: Key,
    value: Rc<Value>,
    next: Option<Rc<RefCell<ListNode<Key, Value>>>>,
    prev: Option<Rc<RefCell<ListNode<Key, Value>>>>,
}

impl<Key:KeyBound, Value:ValueBound> ListNode<Key, Value> {
    fn new(key: Key, value: Value) -> Self {
        Self {
            key: key,
            value: Rc::new(value),
            next: None,
            prev: None,
        }
    }
}

struct DoubleList<K:KeyBound, V:ValueBound> {
    head: Option<Rc<RefCell<ListNode<K, V>>>>,
    tail: Option<Rc<RefCell<ListNode<K, V>>>>,
    len: usize,
}

// only need push_front and remove_tail for now
impl<K:KeyBound, V:ValueBound> DoubleList<K, V> {
    fn new() -> Self {
        Self {
            head: None,
            tail: None,
            len: 0,
        }
    }

    fn push_front(&mut self, node: Rc<RefCell<ListNode<K, V>>>) -> &mut DoubleList<K,V> {
        node.as_ref().borrow_mut().next = self.head.clone();
        match &self.head {
            Some(head) => {
                head.as_ref().borrow_mut().prev = Some(node.clone());
            }
            None => {
                self.tail = Some(node.clone());
            }
        }
        self.len += 1;
        self.head.replace(node);
        self
    }
    fn remove_tail(&mut self) -> Option<Rc<RefCell<ListNode<K, V>>>> {
        match &self.tail {
            Some(tail) => {
                let prev = tail.clone().as_ref().borrow_mut().prev.clone();
                match prev {
                    Some(prev) => {
                        prev.as_ref().borrow_mut().next = None;
                    }
                    None => {
                        self.head = None;
                    }
                }
                self.len -= 1;
                Some(tail.clone())
            }
            None => {
                None
            }
        }
    }
    fn remove_node(&mut self, node: Rc<RefCell<ListNode<K, V>>>) {
        let prev = node.as_ref().borrow().prev.clone();
        let next = node.as_ref().borrow().next.clone();
        match prev.clone() {
            Some(prev) => {
                prev.as_ref().borrow_mut().prev = next.clone();
            }
            None => {
                self.head = next.clone();
            }
        }
        match next {
            Some(next) => {
                next.as_ref().borrow_mut().prev = prev.clone();
            }
            None => {
                self.tail = prev;
            }
        }
        self.len -= 1;
    }
    #[allow(dead_code)]
    fn clear(&mut self) {
        self.head = None;
        self.tail = None;
        self.len = 0;
    }

    fn len(&self) -> usize {
        self.len
    }
}

pub struct LRU<Key: KeyBound, Value: ValueBound> {
    map: HashMap<Key, Rc<RefCell<ListNode<Key, Value>>>>,
    lists: DoubleList<Key, Value>,
    max_size: usize,
}

impl<Key: KeyBound, Value: ValueBound> Cache<Key, Value> for LRU<Key, Value> {
    // if key exists, move the key to the front of the list
    // else return None
    fn get(&mut self, key: &Key) -> Option<Rc<Value>> {
        if self.map.contains_key(key) {
            let node = self.map.get(key).unwrap().clone();
            // let mut node = node.borrow_mut();
            self.lists.remove_node(node.clone());
            self.lists.push_front(node.clone());
            return Some(node.as_ref().borrow().value.clone());
        }
        None
    }

    // if the key exists, remove the node from the list
    // return None if not exists
    fn take(&mut self, key: &Key) -> Option<Value> {
        if self.map.contains_key(key) {
            let node = self.map.remove(key).unwrap();
            // remove the node from the list
            self.lists.remove_node(node.clone());
            let value = Rc::try_unwrap(node).unwrap().into_inner().value;

            return Some(Rc::try_unwrap(value).unwrap());
        }
        None
    }

    // if the key exists, move the key to the front of the list
    // else create a new node at the front of the list
    fn put(&mut self, key: Key, value: Value) {
        if self.lists.len() >= self.max_size {
            self.map.remove(&self.lists.remove_tail().unwrap().as_ref().borrow().key);
        }
        if self.map.contains_key(&key) {
            let node = self.map.get(&key).unwrap();
            node.as_ref().borrow_mut().value = Rc::new(value);
            self.lists.remove_node(node.clone());
            self.lists.push_front(node.clone());
            return;
        } else {
            let node = Rc::new(RefCell::new(ListNode::new(key, value)));
            self.lists.push_front(node.clone());
            self.map.insert(key, node);
        }
    }

    // remove the node both from the map and the list
    fn remove(&mut self, key: &Key) {
        if self.map.contains_key(key) {
            let node: Rc<RefCell<ListNode<Key, Value>>> = self.map.remove(key).unwrap().to_owned();
            self.lists.remove_node(node);
        }
        
    }

    // clear the map and the list
    fn clear(&mut self) {
        self.map.clear();
    }
}



impl<K:KeyBound,V:ValueBound> LRU<K,V> {
    pub fn new(max_size: usize) -> Self {
        Self {
            map: HashMap::new(),
            lists: DoubleList::new(),
            max_size: max_size,
        }
    }
}

use std::sync::{RwLock, Arc};
use std::time::{Duration,Instant};

#[derive(Debug)]
struct Entry<V:ValueBound> {
    value: Rc<V>,
    expire_time: Option<Instant>,
}

#[derive(Debug)]
pub struct TTL<Key:KeyBound, Value:ValueBound> {
    map: HashMap<Key, Entry<Value>>,
    global_ttl: Duration,
}

impl<K:KeyBound,V:ValueBound> Cache<K,V> for TTL<K,V> {
    fn get(&mut self, key: &K) -> Option<Rc<V>> {
        if self.map.contains_key(key) {
            let value = self.map.get(key).unwrap();
            if value.expire_time.is_some() && value.expire_time.unwrap() < Instant::now() {
                self.map.remove(key);
                return None;
            }
            return Some(value.value.clone());
        }
        None
    }

    fn take(&mut self, key: &K) -> Option<V> {
        if self.map.contains_key(key) {
            let value = self.map.remove(key).unwrap();
            if value.expire_time.is_some() && value.expire_time.unwrap() < Instant::now() {
                return None;
            }
            return Some(Rc::try_unwrap(value.value).unwrap());
        }
        None
    }

    fn put(&mut self, key: K, value: V) {
        self.map.insert(key, Entry {
            value: Rc::new(value),
            expire_time: Some(Instant::now() + self.global_ttl),
        });
    }

    fn remove(&mut self, key: &K) {
        self.map.remove(key);
    }

    fn clear(&mut self) {
        self.map.clear();
    }
}


impl<K:KeyBound,V:ValueBound> TTL<K,V> {
    #[allow(dead_code)]
    pub fn new(global_ttl: Duration) -> Self {
        Self {
            map: HashMap::new(),
            global_ttl: global_ttl,
        }
    }
}



pub struct ConcurrentCache<Key: KeyBound, Value: ValueBound> {
    map: Arc<RwLock<HashMap<Key, Rc<Value>>>>,
}

impl<K:KeyBound,V:ValueBound> Cache<K,V> for ConcurrentCache<K,V> {
    fn get(&mut self, key: &K) -> Option<Rc<V>> {
        if self.map.read().unwrap().contains_key(key) {
            return Some(self.map.read().unwrap().get(key).unwrap().clone());
        }
        None
    }

    fn take(&mut self, key: &K) -> Option<V> {
        if self.map.read().unwrap().contains_key(key) {
            let value = self.map.write().unwrap().remove(key).unwrap();
            return Some(Rc::try_unwrap(value).unwrap());
        }
        None
    }

    fn put(&mut self, key: K, value: V) {
        self.map.write().unwrap().insert(key, Rc::new(value));
    }

    fn remove(&mut self, key: &K) {
        self.map.write().unwrap().remove(key);
    }

    fn clear(&mut self) {
        self.map.write().unwrap().clear();
    }
}



#[test]
fn test_lru_cache() {
    let mut cache: LRU<i32, i32> = LRU::new(1);
    cache.put(1, 1);
    cache.put(2, 2);
    assert_eq!(cache.get(&1), None);
}

#[test]
fn test_ttl_cache() {
    let mut cache = TTL::new(Duration::from_secs(1));
    cache.put(1, 1);
    assert_eq!(cache.get(&1), Some(Rc::new(1)));
    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(cache.get(&1), None);
}