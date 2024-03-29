use futures_util::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinHandle;
use zbus::fdo::ObjectManagerProxy;
use zbus::Error::MethodError;
use zbus::{dbus_proxy, Connection};

const RESOURCE_NOT_FOUND_ERR: &str = "xyz.openbmc_project.Common.Error.ResourceNotFound";

#[dbus_proxy(
    interface = "xyz.openbmc_project.ObjectMapper",
    default_service = "xyz.openbmc_project.ObjectMapper",
    default_path = "/xyz/openbmc_project/object_mapper"
)]
trait ObjectMapper {
    async fn get_object(
        &self,
        path: &str,
        interfaces: Vec<&str>,
    ) -> zbus::Result<HashMap<String, Vec<String>>>;
    async fn get_sub_tree_paths(
        &self,
        subtree: &str,
        depth: i32,
        interfaces: Vec<&str>,
    ) -> zbus::Result<Vec<String>>;
}

#[dbus_proxy(
    interface = "xyz.openbmc_project.ObjectMapper.Private",
    default_service = "xyz.openbmc_project.ObjectMapper",
    default_path = "/xyz/openbmc_project/object_mapper"
)]
trait ObjectMapperPrivate {
    #[dbus_proxy(signal)]
    fn introspection_complete(&self, process_name: &str) -> fdo::Result<()>;
}

// Check if the object is valid or RESOURCE_NOT_FOUND_ERR, return the actual
// error otherwise.
fn check_object(
    object: Result<HashMap<String, Vec<String>>, zbus::Error>,
) -> Result<(), Arc<zbus::Error>> {
    if let Err(err) = object {
        let err_og = err.clone();
        if let MethodError(owned_err, _, _) = err {
            if owned_err.as_str() == RESOURCE_NOT_FOUND_ERR {
                return Ok(());
            }
        }
        return Err(Arc::new(err_og));
    }
    return Ok(());
}

pub async fn mapper_wait(obj_path: String) -> Result<(), Box<dyn std::error::Error>> {
    let obj0 = obj_path.clone();
    let obj1 = obj_path.clone();

    let connection = Connection::system().await?;
    let object_mapper_proxy = ObjectMapperProxy::new(&connection).await?;
    let object = object_mapper_proxy
        .get_object(obj0.as_str(), Vec::new())
        .await;
    if let Ok(_) = object {
        return Ok(());
    }
    check_object(object)?;

    let interfaces_added_task: JoinHandle<Result<(), Arc<zbus::Error>>> =
        tokio::spawn(async move {
            let connection = Connection::system().await?;
            let object_manager_proxy = ObjectManagerProxy::builder(&connection)
                .receives_broadcast_signal()
                .build()
                .await?;
            let object_mapper_proxy = ObjectMapperProxy::new(&connection).await?;
            let mut interfaces_added_stream =
                object_manager_proxy.receive_interfaces_added().await?;
            while let Some(_) = interfaces_added_stream.next().await {
                let object = object_mapper_proxy
                    .get_object(obj0.as_str(), Vec::new())
                    .await;
                if let Ok(_) = object {
                    return Ok(());
                };
                check_object(object)?;
            }
            Ok(())
        });

    let introspect_task: JoinHandle<Result<(), Arc<zbus::Error>>> = tokio::spawn(async move {
        let connection = Connection::system().await?;
        let proxy = ObjectMapperPrivateProxy::builder(&connection)
            .receives_broadcast_signal()
            .build()
            .await?;
        let mut introspect_complete_stream = proxy.receive_introspection_complete().await?;
        let object_mapper_proxy = ObjectMapperProxy::new(&connection).await?;
        while let Some(_) = introspect_complete_stream.next().await {
            let object = object_mapper_proxy
                .get_object(obj1.as_str(), Vec::new())
                .await;
            if let Ok(_) = object {
                return Ok(());
            }
            check_object(object)?;
        }
        return Ok(());
    });

    // Check both async task and exit program if any of the task return Ok or
    // Errors out.
    loop {
        if interfaces_added_task.is_finished() {
            interfaces_added_task.await??;
            return Ok(());
        }
        if introspect_task.is_finished() {
            introspect_task.await??;
            return Ok(());
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}

fn check_subtree_paths(
    subtree_paths: Result<Vec<String>, zbus::Error>,
) -> Result<bool, Arc<zbus::Error>> {
    if let Ok(subtree_paths) = subtree_paths {
        if !subtree_paths.is_empty() {
            return Ok(false);
        }
        // Found the namespace, but interface filter returned nothing.
        return Ok(true);
    }
    if let Err(err) = subtree_paths {
        let err_og = err.clone();
        if let MethodError(owned_err, _, _) = err {
            // The interface is removed if we hit ResourceNotFound under the namespace.
            if owned_err.as_str() == RESOURCE_NOT_FOUND_ERR {
                return Ok(true);
            }
        }
        return Err(Arc::new(err_og));
    }

    Ok(false)
}

async fn check_subtree_interface_removed(
    namespace: &str,
    interface: &str,
) -> Result<bool, Arc<zbus::Error>> {
    let connection = Connection::system().await?;
    let object_mapper_proxy = ObjectMapperProxy::new(&connection).await?;
    let subtree_paths = object_mapper_proxy
        .get_sub_tree_paths(namespace, 0, vec![interface])
        .await;
    check_subtree_paths(subtree_paths)?;
    Ok(true)
}

pub async fn mapper_subtree_remove(
    namespace: &str,
    interface: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if check_subtree_interface_removed(namespace, interface).await? {
        return Ok(());
    }

    let connection = Connection::system().await?;
    let proxy = ObjectManagerProxy::builder(&connection)
        .receives_broadcast_signal()
        .build()
        .await?;
    let mut interface_removed_stream = proxy.receive_interfaces_removed().await?;
    while let Some(_) = interface_removed_stream.next().await {
        if !check_subtree_interface_removed(namespace, interface).await? {
            continue;
        }

        return Ok(());
    }
    Ok(())
}

pub async fn mapper_get_service(
    obj_path: String,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let connection = Connection::system().await?;
    let proxy = ObjectMapperProxy::new(&connection).await?;
    let object = proxy.get_object(obj_path.as_str(), Vec::new()).await?;
    let mut output: Vec<String> = Vec::new();
    for (key, _) in object {
        output.push(key);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Message;
    use zbus_names::OwnedErrorName;

    fn create_test_msg() -> Arc<Message> {
        let raw_body: &[u8] = &[16, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0];
        let message_builder = zbus::MessageBuilder::signal("/", "test.test", "test").unwrap();
        let message = unsafe {
            message_builder
                .build_raw_body(
                    raw_body,
                    "ai",
                    #[cfg(unix)]
                    vec![],
                )
                .unwrap()
        };
        Arc::new(message)
    }

    mod check_object {
        use super::*;
        #[test]
        fn test_valid_object() -> Result<(), Arc<zbus::Error>> {
            let object = Ok(HashMap::new());
            check_object(object)?;
            Ok(())
        }

        #[test]
        fn test_resource_not_found_error() -> Result<(), Arc<zbus::Error>> {
            let err_name = OwnedErrorName::try_from(RESOURCE_NOT_FOUND_ERR).unwrap();
            let object = Err(zbus::Error::MethodError(err_name, None, create_test_msg()));
            check_object(object)?;
            Ok(())
        }

        #[test]
        fn test_different_error() {
            let object = Err(zbus::Error::InterfaceNotFound);
            assert!(
                check_object(object).is_err_and(|e| e == Arc::new(zbus::Error::InterfaceNotFound))
            );
        }

        #[test]
        fn test_invalid_method_error() {
            let err_name = OwnedErrorName::try_from("test.error").unwrap();
            let err_name_copy = err_name.clone();
            let object = Err(zbus::Error::MethodError(err_name, None, create_test_msg()));
            assert!(check_object(object).is_err_and(|e| {
                e == Arc::new(zbus::Error::MethodError(
                    err_name_copy,
                    None,
                    create_test_msg(),
                ))
            }));
        }
    }

    mod check_subtree_paths {
        use super::*;

        #[test]
        fn test_empty_subtree_paths() -> Result<(), Arc<zbus::Error>> {
            let subtree_paths = Vec::new();
            assert_eq!(check_subtree_paths(Ok(subtree_paths))?, true);
            Ok(())
        }

        #[test]
        fn test_nonempty_subtree_paths() -> Result<(), Arc<zbus::Error>> {
            let subtree_paths: Vec<String> = vec![String::from("123")];
            assert_eq!(check_subtree_paths(Ok(subtree_paths))?, false);
            Ok(())
        }

        #[test]
        fn test_resource_not_found_error() -> Result<(), Arc<zbus::Error>> {
            let err_name = OwnedErrorName::try_from(RESOURCE_NOT_FOUND_ERR).unwrap();
            let subtree_paths = Err(zbus::Error::MethodError(err_name, None, create_test_msg()));
            assert_eq!(check_subtree_paths(subtree_paths)?, true);
            Ok(())
        }

        #[test]
        fn test_different_error() {
            let subtree_paths = Err(zbus::Error::InterfaceNotFound);
            assert!(check_subtree_paths(subtree_paths)
                .is_err_and(|e| e == Arc::new(zbus::Error::InterfaceNotFound)));
        }

        #[test]
        fn test_invalid_method_error() {
            let err_name = OwnedErrorName::try_from("test.error").unwrap();
            let err_name_copy = err_name.clone();
            let subtree_paths = Err(zbus::Error::MethodError(err_name, None, create_test_msg()));
            assert!(check_subtree_paths(subtree_paths).is_err_and(|e| {
                e == Arc::new(zbus::Error::MethodError(
                    err_name_copy,
                    None,
                    create_test_msg(),
                ))
            }));
        }
    }
}
