use crate::discovery::WorkspaceMember;
use crate::error::WorkspaceError;

/// Resolve which workspace members to scan based on `--project` filter.
///
/// Returns `Ok(filtered_members)` or `Err` with the unknown member name.
pub fn resolve_members<'a>(
    members: &'a [WorkspaceMember],
    project_filter: &[String],
) -> Result<Vec<&'a WorkspaceMember>, WorkspaceError> {
    if project_filter.is_empty() {
        // Scan all members
        return Ok(members.iter().collect());
    }

    // Validate that all requested names exist
    let available: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
    let mut selected = Vec::new();

    for name in project_filter {
        if let Some(member) = members.iter().find(|m| m.name == *name) {
            selected.push(member);
        } else {
            return Err(WorkspaceError::UnknownMember {
                name: name.clone(),
                available: available.join(", "),
            });
        }
    }

    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_members() -> Vec<WorkspaceMember> {
        vec![
            WorkspaceMember {
                name: "core".into(),
                root_dir: PathBuf::from("/ws/core"),
            },
            WorkspaceMember {
                name: "api".into(),
                root_dir: PathBuf::from("/ws/api"),
            },
            WorkspaceMember {
                name: "web".into(),
                root_dir: PathBuf::from("/ws/web"),
            },
        ]
    }

    #[test]
    fn test_resolve_all_members() {
        let members = make_members();
        let result = resolve_members(&members, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 3);
    }

    #[test]
    fn test_resolve_specific_members() {
        let members = make_members();
        let filter = vec!["core".into(), "api".into()];
        let result = resolve_members(&members, &filter);
        assert!(result.is_ok());
        let selected = result.unwrap();
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].name, "core");
        assert_eq!(selected[1].name, "api");
    }

    #[test]
    fn test_resolve_unknown_member() {
        let members = make_members();
        let filter = vec!["nonexistent".into()];
        let result = resolve_members(&members, &filter);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nonexistent"));
        assert!(err.contains("core"));
        assert!(err.contains("api"));
        assert!(err.contains("web"));
    }

    #[test]
    fn test_resolve_single_member() {
        let members = make_members();
        let filter = vec!["web".into()];
        let result = resolve_members(&members, &filter);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
