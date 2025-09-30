//! Example demonstrating the use of NonEmpty collections for type-safe APIs.
//!
//! This example shows how NonEmptyVec and NonEmptyQueue prevent invalid states
//! at compile time, making APIs safer and more ergonomic.

use skreaver_core::collections::{NonEmptyQueue, NonEmptyVec};

fn main() {
    println!("=== NonEmptyVec Examples ===\n");
    nonempty_vec_examples();

    println!("\n=== NonEmptyQueue Examples ===\n");
    nonempty_queue_examples();

    println!("\n=== Type Safety Examples ===\n");
    type_safety_examples();
}

fn nonempty_vec_examples() {
    // Creating a NonEmptyVec
    let vec = NonEmptyVec::new(1, vec![2, 3, 4, 5]);
    println!("Created NonEmptyVec: {}", vec);
    println!("  Head: {}", vec.head());
    println!("  Tail: {:?}", vec.tail());
    println!("  Length: {}", vec.len());

    // Singleton
    let single = NonEmptyVec::singleton("Hello");
    println!("\nSingleton: {}", single);
    println!("  Is singleton: {}", single.is_singleton());

    // Operations that preserve non-emptiness
    let mut mutable_vec = NonEmptyVec::new(10, vec![20, 30]);
    mutable_vec.push(40);
    println!("\nAfter push: {}", mutable_vec);

    let popped = mutable_vec.pop();
    println!("Popped: {:?}", popped);
    println!("After pop: {}", mutable_vec);

    // Can't pop the last element - this returns None
    let mut singleton = NonEmptyVec::singleton(42);
    let result = singleton.pop();
    println!("\nTrying to pop from singleton: {:?}", result);
    println!("Singleton still has: {}", singleton.head());

    // Iteration
    let vec = NonEmptyVec::new("first", vec!["second", "third"]);
    print!("\nIteration: ");
    for item in vec.iter() {
        print!("{} ", item);
    }
    println!();

    // Conversion
    let regular_vec: Vec<_> = NonEmptyVec::new(1, vec![2, 3]).into_vec();
    println!("\nConverted to Vec: {:?}", regular_vec);
}

fn nonempty_queue_examples() {
    // Creating a NonEmptyQueue
    let queue = NonEmptyQueue::new("task1", vec!["task2", "task3"]);
    println!("Created NonEmptyQueue: {}", queue);
    println!("  Front: {}", queue.peek());
    println!("  Back: {}", queue.back());
    println!("  Length: {}", queue.len());

    // FIFO operations
    let mut task_queue = NonEmptyQueue::new("critical", vec!["high", "medium"]);
    task_queue.enqueue("low");
    println!("\nTask queue after enqueue: {}", task_queue);

    if let Some(task) = task_queue.dequeue() {
        println!("Processing task: {}", task);
    }
    println!("Queue after dequeue: {}", task_queue);

    // Can't dequeue the last task - this returns None
    let mut single_task = NonEmptyQueue::singleton("last_task");
    let result = single_task.dequeue();
    println!("\nTrying to dequeue singleton: {:?}", result);
    println!("Still has task: {}", single_task.peek());

    // Iteration in queue order
    let queue = NonEmptyQueue::new(1, vec![2, 3, 4]);
    print!("\nQueue iteration: ");
    for item in queue.iter() {
        print!("{} ", item);
    }
    println!();
}

fn type_safety_examples() {
    // Type-safe API that requires at least one element
    fn process_batch(items: NonEmptyVec<String>) -> String {
        // We can safely access head without Option
        let first = items.head();
        format!("Processing {} items starting with '{}'", items.len(), first)
    }

    let batch = NonEmptyVec::new("item1".to_string(), vec!["item2".to_string()]);
    println!("{}", process_batch(batch));

    // Type-safe tool execution pipeline
    fn execute_tools(mut queue: NonEmptyQueue<&str>) -> Vec<String> {
        let mut results = Vec::new();

        // Process all tools in order
        loop {
            let tool = queue.peek();
            results.push(format!("Executed: {}", tool));

            // Try to move to next tool
            if queue.dequeue().is_none() {
                // Last tool processed
                break;
            }
        }

        results
    }

    let tools = NonEmptyQueue::new("http_get", vec!["json_parse", "text_transform"]);
    let results = execute_tools(tools);
    println!("\nTool execution results:");
    for result in results {
        println!("  {}", result);
    }

    // Compile-time prevention of empty collections
    // This won't compile:
    // let empty_vec: Vec<i32> = vec![];
    // let invalid = NonEmptyVec::try_from(empty_vec).unwrap(); // Would panic

    // Instead, use TryFrom for runtime validation:
    let data = vec![1, 2, 3];
    match NonEmptyVec::try_from(data) {
        Ok(non_empty) => println!(
            "\nSuccessfully created NonEmptyVec with {} items",
            non_empty.len()
        ),
        Err(e) => println!("\nFailed to create NonEmptyVec: {}", e),
    }

    // Or use the constructor for known non-empty data:
    let guaranteed = NonEmptyVec::singleton(42);
    println!("Guaranteed non-empty: {}", guaranteed);
}
