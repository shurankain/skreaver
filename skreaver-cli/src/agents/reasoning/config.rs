/// Configuration profile for reasoning behavior
#[derive(Clone, Debug)]
pub struct ReasoningProfile {
    pub max_loop_iters: usize,
    pub max_prev_output: usize,
    pub max_chain_line: usize,
    pub max_chain_summary: usize,
}

impl Default for ReasoningProfile {
    fn default() -> Self {
        Self {
            max_loop_iters: 16,
            max_prev_output: 1024,
            max_chain_line: 512,
            max_chain_summary: 2048,
        }
    }
}

impl ReasoningProfile {
    /// Create a high-performance profile for fast reasoning.
    #[cfg(test)]
    pub fn fast() -> Self {
        Self {
            max_loop_iters: 8,
            max_prev_output: 512,
            max_chain_line: 256,
            max_chain_summary: 1024,
        }
    }

    /// Create a comprehensive profile for thorough reasoning.
    #[cfg(test)]
    pub fn comprehensive() -> Self {
        Self {
            max_loop_iters: 32,
            max_prev_output: 4096,
            max_chain_line: 1024,
            max_chain_summary: 8192,
        }
    }
}
