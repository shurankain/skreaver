//! A2A protocol adapter for the unified agent interface.
//!
//! This module provides adapters to use A2A agents through the unified interface,
//! as well as conversion functions between unified and A2A types.

mod adapter;
pub mod conversions;

pub use adapter::A2aAgentAdapter;

// Re-export commonly used conversion functions
pub use conversions::{
    a2a_card_to_agent_info, a2a_part_to_content_part, a2a_to_unified_artifact,
    a2a_to_unified_message, a2a_to_unified_role, a2a_to_unified_status,
    a2a_to_unified_stream_event, a2a_to_unified_task, unified_content_to_a2a_parts,
    unified_info_to_a2a_card, unified_to_a2a_artifact, unified_to_a2a_message, unified_to_a2a_role,
    unified_to_a2a_status, unified_to_a2a_stream_event, update_a2a_task_from_unified,
};
