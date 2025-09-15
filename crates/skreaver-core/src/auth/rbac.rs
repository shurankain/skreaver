//! Role-Based Access Control (RBAC) system

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;

/// System roles
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    /// Full system access
    Admin,
    /// Agent role - can execute tools and access memory
    Agent,
    /// Read-only access
    Viewer,
    /// Custom role (for extension)
    Custom(String),
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Agent => write!(f, "agent"),
            Role::Viewer => write!(f, "viewer"),
            Role::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl Role {
    /// Get all permissions for this role
    pub fn permissions(&self) -> HashSet<Permission> {
        match self {
            Role::Admin => {
                // Admin has all permissions
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadMemory);
                perms.insert(Permission::WriteMemory);
                perms.insert(Permission::ExecuteTool);
                perms.insert(Permission::ManageAgents);
                perms.insert(Permission::ManageAuth);
                perms.insert(Permission::ViewMetrics);
                perms.insert(Permission::ModifyConfig);
                perms.insert(Permission::AccessAdmin);
                perms
            }
            Role::Agent => {
                // Agent can read/write memory and execute tools
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadMemory);
                perms.insert(Permission::WriteMemory);
                perms.insert(Permission::ExecuteTool);
                perms.insert(Permission::ViewMetrics);
                perms
            }
            Role::Viewer => {
                // Viewer has read-only access
                let mut perms = HashSet::new();
                perms.insert(Permission::ReadMemory);
                perms.insert(Permission::ViewMetrics);
                perms
            }
            Role::Custom(_) => {
                // Custom roles start with no permissions
                HashSet::new()
            }
        }
    }

    /// Check if role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        self.permissions().contains(permission)
    }
}

/// System permissions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    /// Read from memory backend
    ReadMemory,
    /// Write to memory backend
    WriteMemory,
    /// Execute tools
    ExecuteTool,
    /// Manage agent lifecycle
    ManageAgents,
    /// Manage authentication and authorization
    ManageAuth,
    /// View system metrics
    ViewMetrics,
    /// Modify system configuration
    ModifyConfig,
    /// Access admin endpoints
    AccessAdmin,
    /// Custom permission (for extension)
    Custom(String),
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::ReadMemory => write!(f, "memory:read"),
            Permission::WriteMemory => write!(f, "memory:write"),
            Permission::ExecuteTool => write!(f, "tool:execute"),
            Permission::ManageAgents => write!(f, "agent:manage"),
            Permission::ManageAuth => write!(f, "auth:manage"),
            Permission::ViewMetrics => write!(f, "metrics:view"),
            Permission::ModifyConfig => write!(f, "config:modify"),
            Permission::AccessAdmin => write!(f, "admin:access"),
            Permission::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// Tool access policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolPolicy {
    /// Tool name pattern (supports wildcards)
    pub tool_pattern: String,
    /// Required roles to access this tool
    pub required_roles: HashSet<Role>,
    /// Required permissions to access this tool
    pub required_permissions: HashSet<Permission>,
    /// Is this tool blocked entirely?
    pub blocked: bool,
}

impl ToolPolicy {
    /// Create a new tool policy
    pub fn new(tool_pattern: String) -> Self {
        Self {
            tool_pattern,
            required_roles: HashSet::new(),
            required_permissions: HashSet::new(),
            blocked: false,
        }
    }

    /// Require a role for this tool
    pub fn require_role(mut self, role: Role) -> Self {
        self.required_roles.insert(role);
        self
    }

    /// Require a permission for this tool
    pub fn require_permission(mut self, permission: Permission) -> Self {
        self.required_permissions.insert(permission);
        self
    }

    /// Block this tool entirely
    pub fn block(mut self) -> Self {
        self.blocked = true;
        self
    }

    /// Check if a tool name matches this policy
    pub fn matches(&self, tool_name: &str) -> bool {
        if self.tool_pattern == "*" {
            return true;
        }

        if self.tool_pattern.ends_with('*') {
            let prefix = &self.tool_pattern[..self.tool_pattern.len() - 1];
            return tool_name.starts_with(prefix);
        }

        self.tool_pattern == tool_name
    }

    /// Check if roles and permissions satisfy this policy
    pub fn is_allowed(&self, roles: &[Role], permissions: &HashSet<Permission>) -> bool {
        if self.blocked {
            return false;
        }

        // Check if any required role is present
        let has_required_role =
            self.required_roles.is_empty() || self.required_roles.iter().any(|r| roles.contains(r));

        // Check if all required permissions are present
        let has_required_permissions = self.required_permissions.is_empty()
            || self
                .required_permissions
                .iter()
                .all(|p| permissions.contains(p));

        has_required_role && has_required_permissions
    }
}

/// Role manager for RBAC
pub struct RoleManager {
    /// Tool access policies
    tool_policies: Vec<ToolPolicy>,
    /// Custom role definitions
    custom_roles: std::collections::HashMap<String, HashSet<Permission>>,
}

impl RoleManager {
    /// Create a new role manager
    pub fn new() -> Self {
        Self {
            tool_policies: Vec::new(),
            custom_roles: std::collections::HashMap::new(),
        }
    }

    /// Create with default policies
    pub fn with_defaults() -> Self {
        let mut manager = Self::new();

        // Add default tool policies
        // Dangerous tools require admin role
        manager.add_tool_policy(ToolPolicy::new("shell_*".to_string()).require_role(Role::Admin));

        manager
            .add_tool_policy(ToolPolicy::new("file_delete".to_string()).require_role(Role::Admin));

        // Read-only tools available to viewers
        manager.add_tool_policy(
            ToolPolicy::new("http_get".to_string()).require_permission(Permission::ExecuteTool),
        );

        manager
    }

    /// Add a tool policy
    pub fn add_tool_policy(&mut self, policy: ToolPolicy) {
        self.tool_policies.push(policy);
    }

    /// Define a custom role
    pub fn define_custom_role(&mut self, name: String, permissions: HashSet<Permission>) {
        self.custom_roles.insert(name, permissions);
    }

    /// Check if a tool can be accessed
    pub fn check_tool_access(
        &self,
        tool_name: &str,
        roles: &[Role],
        permissions: &HashSet<Permission>,
    ) -> bool {
        // Find all matching policies
        let matching_policies: Vec<_> = self
            .tool_policies
            .iter()
            .filter(|p| p.matches(tool_name))
            .collect();

        // If no policies match, default to checking ExecuteTool permission
        if matching_policies.is_empty() {
            return permissions.contains(&Permission::ExecuteTool);
        }

        // All matching policies must allow access
        matching_policies
            .iter()
            .all(|p| p.is_allowed(roles, permissions))
    }

    /// Get permissions for a custom role
    pub fn get_custom_role_permissions(&self, role_name: &str) -> Option<&HashSet<Permission>> {
        self.custom_roles.get(role_name)
    }
}

impl Default for RoleManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_permissions() {
        let admin = Role::Admin;
        assert!(admin.has_permission(&Permission::ManageAuth));
        assert!(admin.has_permission(&Permission::ExecuteTool));

        let agent = Role::Agent;
        assert!(agent.has_permission(&Permission::ExecuteTool));
        assert!(!agent.has_permission(&Permission::ManageAuth));

        let viewer = Role::Viewer;
        assert!(viewer.has_permission(&Permission::ReadMemory));
        assert!(!viewer.has_permission(&Permission::WriteMemory));
    }

    #[test]
    fn test_tool_policy() {
        let policy = ToolPolicy::new("dangerous_*".to_string()).require_role(Role::Admin);

        assert!(policy.matches("dangerous_command"));
        assert!(policy.matches("dangerous_delete"));
        assert!(!policy.matches("safe_command"));

        let admin_roles = vec![Role::Admin];
        let admin_perms = Role::Admin.permissions();
        assert!(policy.is_allowed(&admin_roles, &admin_perms));

        let agent_roles = vec![Role::Agent];
        let agent_perms = Role::Agent.permissions();
        assert!(!policy.is_allowed(&agent_roles, &agent_perms));
    }

    #[test]
    fn test_role_manager() {
        let manager = RoleManager::with_defaults();

        let admin_roles = vec![Role::Admin];
        let admin_perms = Role::Admin.permissions();

        // Admin should access shell commands
        assert!(manager.check_tool_access("shell_exec", &admin_roles, &admin_perms));

        let agent_roles = vec![Role::Agent];
        let agent_perms = Role::Agent.permissions();

        // Agent should not access shell commands
        assert!(!manager.check_tool_access("shell_exec", &agent_roles, &agent_perms));

        // Agent should access regular tools
        assert!(manager.check_tool_access("http_get", &agent_roles, &agent_perms));
    }
}
