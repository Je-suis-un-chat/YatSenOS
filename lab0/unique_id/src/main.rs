use std::sync::atomic::{AtomicU16, Ordering};

#[derive(Debug,PartialEq)]
pub struct UniqueId(u16);

impl UniqueId{

    pub fn new() -> Self {
        static NEXT_ID: AtomicU16 = AtomicU16::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        UniqueId(id)
    }
} 

fn main() {
    let id1 = UniqueId::new();
    let id2 = UniqueId::new();
    println!("Generated Unique IDs: {:?}, {:?}", id1, id2);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_unique_id_generation() {
        let id1 = UniqueId::new();
        let id2 = UniqueId::new();
        assert_ne!(id1, id2, "Unique IDs should be different");
    }
}