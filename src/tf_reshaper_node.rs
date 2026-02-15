use anyhow::{Error, Result};
use rclrs::*;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_yaml;

use tf2_rs::broadcaster::TransformBroadcaster;
use tf2_rs::listener::TransformListener;
use tf2_rs::{BufferCore, LookupTime};

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Remapping {
    old_parent: String,
    old_child: String,
    new_parent: String,
    new_child: String,
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct RemapConfig {
    remaps: Vec<Remapping>,
}

fn main() -> Result<(), Error> {
    let context = Context::default_from_env()?;
    let mut executor = context.create_basic_executor();

    let node = executor.create_node("tf_reshaper")?;

    let buffer = BufferCore::new(std::time::Duration::new(10, 0));
    let _listener = TransformListener::new(&node, buffer.clone())?;
    let broadcaster = TransformBroadcaster::new(&node)?;

    let config_path: MandatoryParameter<Arc<str>> = node
        .declare_parameter("config_path")
        .default("test.yaml".into())
        .mandatory()?;

    let f = std::fs::File::open(&*config_path.get())?;
    let remap_config: RemapConfig = serde_yaml::from_reader(f)?;

    let reshape_fn = move || {
        remap_config.remaps.iter().for_each(|remap| {
            match buffer.lookup_transform(
                remap.old_parent.as_str(),
                remap.old_child.as_str(),
                LookupTime::Latest,
            ) {
                Ok(mut transform) => {
                    transform.parent_frame = remap.new_parent.clone();
                    transform.child_frame = remap.new_child.clone();
                    if let Err(e) = broadcaster.send_transform(transform) {
                        eprintln!("Failed to boradcast transform: {:?}", e);
                    }
                }
                Err(e) => {
                    eprintln!("TF lookup failed: {:?}", e);
                }
            }
        })
    };
    node.create_timer_repeating(
        TimerOptions::new(Duration::from_secs(1)).node_time(),
        reshape_fn,
    )?;

    println!("Waiting for messages...");
    executor.spin(SpinOptions::default()).first_error()?;
    Ok(())
}
