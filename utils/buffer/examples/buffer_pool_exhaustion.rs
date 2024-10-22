// # Handling Buffer Pool Exhaustion and Heap Allocation
//
// 1. Write bytes to fill up back of buffer pool.
// 2. Expect pool mode will switch to front on next write.
// 3. Write bytes to full up front of buffer pool.
// 4. Expect pool mode will switch to alloc on next write.
// 5. Write bytes bigger than buffer pool capacity so that it is allocated on the heap.

use buffer_sv2::{Buffer, BufferPool};

fn main() {
    let mut buffer_pool = BufferPool::new(8); // 8 bytes capacity

    // Write data to the buffer and retrieve it
    let write_and_retrieve = |pool: &mut BufferPool<_>, data: &[u8]| {
        let writable = pool.get_writable(data.len());
        writable.copy_from_slice(data);
        println!("Buffer Pool: {:?}", &pool);
        let data_slice = pool.get_data_by_ref(data.len());
        println!("Slice from Buffer Pool: {:?}", data_slice.as_ref());
        // data_slice goes out of scope here
    };

    let data_bytes = b"abcd"; // 4 bytes
    println!("DATA: {:?}", data_bytes);
    write_and_retrieve(&mut buffer_pool, data_bytes);
    assert!(&buffer_pool.is_back_mode());
    println!("");

    let data_bytes = b"efg"; // 3 bytes
    println!("DATA: {:?}", data_bytes);
    write_and_retrieve(&mut buffer_pool, data_bytes);
    assert!(&buffer_pool.is_front_mode());
    println!("");

    let data_bytes = b"hijkl"; // 5 bytes
    println!("DATA: {:?}", data_bytes);
    write_and_retrieve(&mut buffer_pool, data_bytes);
    assert!(&buffer_pool.is_alloc_mode());
}
