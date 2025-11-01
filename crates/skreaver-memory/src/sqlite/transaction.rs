//! TransactionalMemory implementation for SqliteMemory

use skreaver_core::error::TransactionError;
use skreaver_core::memory::{MemoryWriter, TransactionalMemory};

use super::SqliteMemory;

impl TransactionalMemory for SqliteMemory {
    fn transaction<F, R>(&mut self, f: F) -> Result<R, TransactionError>
    where
        F: FnOnce(&mut dyn MemoryWriter) -> Result<R, TransactionError>,
    {
        // Generate unique savepoint name to avoid conflicts
        let savepoint_name = format!("sp_{}", rand::random::<u32>());

        // Get a connection and start savepoint
        let mut conn = self
            .pool
            .acquire()
            .map_err(|e| TransactionError::TransactionFailed {
                reason: format!("Failed to acquire connection for transaction: {}", e),
            })?;

        // Begin savepoint for transaction isolation
        conn.as_mut()
            .execute(&format!("SAVEPOINT {}", savepoint_name), [])
            .map_err(|e| TransactionError::TransactionFailed {
                reason: format!("Failed to begin transaction savepoint: {}", e),
            })?;

        // Drop connection so pool can be used by operations within transaction
        drop(conn);

        // Execute the transaction function
        let result = f(self);

        // Reacquire connection to commit/rollback
        let mut conn = self
            .pool
            .acquire()
            .map_err(|e| TransactionError::TransactionFailed {
                reason: format!(
                    "Failed to reacquire connection for transaction commit: {}",
                    e
                ),
            })?;

        match result {
            Ok(value) => {
                // Release the savepoint (commit)
                conn.as_mut()
                    .execute(&format!("RELEASE SAVEPOINT {}", savepoint_name), [])
                    .map_err(|e| TransactionError::TransactionFailed {
                        reason: format!("Failed to commit transaction: {}", e),
                    })?;
                Ok(value)
            }
            Err(tx_error) => {
                // Rollback to savepoint
                if let Err(rollback_err) = conn
                    .as_mut()
                    .execute(&format!("ROLLBACK TO SAVEPOINT {}", savepoint_name), [])
                {
                    eprintln!("Warning: Failed to rollback transaction: {}", rollback_err);
                }
                // Also release the savepoint after rollback
                let _ = conn
                    .as_mut()
                    .execute(&format!("RELEASE SAVEPOINT {}", savepoint_name), []);
                Err(tx_error)
            }
        }
    }
}
