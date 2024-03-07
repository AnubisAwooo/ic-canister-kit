use std::collections::{HashMap, HashSet};

use crate::{
    functions::types::{Permissable, Permission, PermissionUpdatedArg, PermissionUpdatedError},
    identity::UserId,
};

// ================== 简单实现 ==================

// 多个权限对象
#[derive(candid::CandidType, serde::Deserialize, Debug, Default)]
pub struct Permissions {
    pub permissions: HashSet<Permission>, // 所有权限
    pub user_permissions: HashMap<UserId, HashSet<Permission>>, // 用户分配的特别权限, Permitted表示拥有, Forbidden表示禁止
    pub role_permissions: HashMap<String, HashSet<Permission>>, // 某角色对权限的限制
    pub user_roles: HashMap<UserId, HashSet<String>>,           // 用户所拥有的角色
}

impl Permissions {
    // 检查一定存在权限
    fn assure_permission_exist(
        &self,
        permissions: &Option<HashSet<Permission>>,
    ) -> Result<(), PermissionUpdatedError> {
        if let Some(permissions) = permissions {
            for permission in permissions {
                if !self.permissions.contains(permission) {
                    return Err(PermissionUpdatedError::InvalidPermission(
                        permission.clone(),
                    ));
                }
            }
        }

        Ok(())
    }
    // 检查一定存在角色
    fn assure_role_exist(
        &self,
        roles: &Option<HashSet<String>>,
    ) -> Result<(), PermissionUpdatedError> {
        if let Some(roles) = roles {
            for role in roles {
                if !self.role_permissions.contains_key(role) {
                    return Err(PermissionUpdatedError::InvalidRole(role.clone()));
                }
            }
        }
        Ok(())
    }
}

impl Permissable for Permissions {
    // 查询
    fn permission_users(&self) -> HashSet<&UserId> {
        let mut users: HashSet<&UserId> = self.user_roles.keys().collect();
        users.extend(self.user_permissions.keys());
        users
    }
    fn permission_roles(&self) -> HashSet<&String> {
        self.role_permissions.keys().collect()
    }

    fn permission_assigned(&self, user_id: &UserId) -> Option<&HashSet<Permission>> {
        self.user_permissions.get(user_id)
    }
    fn permission_role_assigned(&self, role: &str) -> Option<&HashSet<Permission>> {
        self.role_permissions.get(role)
    }
    fn permission_user_roles(&self, user_id: &UserId) -> Option<&HashSet<String>> {
        self.user_roles.get(user_id)
    }

    fn permission_has(&self, user_id: &UserId, permission: &Permission) -> bool {
        // 单独指定
        if let Some(permissions) = self.user_permissions.get(user_id) {
            if permissions.contains(permission) {
                return match permission {
                    Permission::Permitted(_) => true,
                    Permission::Forbidden(_) => false,
                };
            }
        }
        // 角色自定
        if let Some(roles) = self.user_roles.get(user_id) {
            for role in roles {
                if let Some(permissions) = self.role_permissions.get(role) {
                    if permissions.contains(permission) {
                        return match permission {
                            Permission::Permitted(_) => true,
                            Permission::Forbidden(_) => false,
                        };
                    }
                }
            }
        }
        // 不存在则默认
        match permission {
            Permission::Permitted(_) => false,
            Permission::Forbidden(_) => true,
        }
    }
    fn permission_owned(&self, user_id: &UserId) -> HashMap<&Permission, bool> {
        self.permissions
            .iter()
            .map(|permission| (permission, self.permission_has(user_id, permission)))
            .collect()
    }

    // 修改
    fn permission_reset(&mut self, permissions: HashSet<Permission>) {
        self.permissions = permissions;
        // 核对其他数据中的权限是否正确
        self.role_permissions
            .iter_mut()
            .for_each(|(_, permissions)| {
                let mut removed = Vec::new();
                for permission in permissions.iter() {
                    if !self.permissions.contains(permission) {
                        removed.push(permission.clone());
                    }
                }
                for permission in removed {
                    permissions.remove(&permission);
                }
            });
        self.user_permissions
            .iter_mut()
            .for_each(|(_, permissions)| {
                let mut removed = Vec::new();
                for permission in permissions.iter() {
                    if !self.permissions.contains(permission) {
                        removed.push(permission.clone());
                    }
                }
                for permission in removed {
                    permissions.remove(&permission);
                }
            });
    }
    fn permission_update(
        &mut self,
        args: Vec<PermissionUpdatedArg>,
    ) -> Result<(), PermissionUpdatedError> {
        for arg in args.iter() {
            match arg {
                PermissionUpdatedArg::UpdateUserPermission(user_id, permissions) => {
                    // 先检查权限是否都存在
                    self.assure_permission_exist(permissions)?;

                    let exist = self.user_permissions.get(user_id);
                    if let Some(permissions) = &permissions {
                        if let Some(exist) = exist {
                            if exist == permissions {
                                continue;
                            }
                        }
                    } else if exist.is_none() {
                        continue;
                    }
                    if let Some(permissions) = permissions {
                        self.user_permissions.insert(*user_id, permissions.clone());
                    } else {
                        self.user_permissions.remove(user_id);
                    }
                }
                PermissionUpdatedArg::UpdateRolePermission(role, permissions) => {
                    // 先检查权限是否都存在
                    self.assure_permission_exist(permissions)?;

                    let exist = self.role_permissions.get(role);
                    if let Some(permissions) = permissions {
                        if let Some(exist) = exist {
                            if exist == permissions {
                                continue;
                            }
                        }
                    } else if exist.is_none() {
                        continue;
                    }
                    if let Some(permissions) = permissions {
                        self.role_permissions
                            .insert(role.clone(), permissions.clone());
                    } else {
                        self.role_permissions.remove(role);
                        // 移除要检查用户角色数据对不对
                        self.user_roles.iter_mut().for_each(|(_, roles)| {
                            let mut removed = Vec::new();
                            for role in roles.iter() {
                                if !self.role_permissions.contains_key(role) {
                                    removed.push(role.clone());
                                }
                            }
                            for role in removed {
                                roles.remove(&role);
                            }
                        });
                    }
                }
                PermissionUpdatedArg::UpdateUserRole(user_id, roles) => {
                    // 先检查角色是否都存在
                    self.assure_role_exist(roles)?;

                    let exist = self.user_roles.get(user_id);
                    if let Some(roles) = &roles {
                        if let Some(exist) = exist {
                            if exist == roles {
                                continue;
                            }
                        }
                    } else if exist.is_none() {
                        continue;
                    }
                    if let Some(roles) = roles {
                        self.user_roles.insert(*user_id, roles.clone());
                    } else {
                        self.user_roles.remove(user_id);
                    }
                }
            }
        }
        Ok(())
    }
}