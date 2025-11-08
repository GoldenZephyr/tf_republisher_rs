use anyhow::{Error, Result};
use geometry_msgs::msg::TransformStamped;
use rclrs::*;
use std::collections::HashMap;
use std::sync::Arc;
use tf2_msgs::msg::TFMessage;

use serde::{Deserialize, Serialize};
use serde_yaml;

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

    let node = executor.create_node("tf_republisher")?;

    let publisher = node.create_publisher::<TFMessage>("/tf")?;
    let static_publisher = node.create_publisher::<TFMessage>("/tf_static".transient_local())?;

    let config_path: MandatoryParameter<Arc<str>> = node
        .declare_parameter("config_path")
        .default("test.yaml".into())
        .mandatory()?;

    let f = std::fs::File::open(&*config_path.get())?;
    let remap_config: RemapConfig = serde_yaml::from_reader(f)?;

    let boxed_config = Box::leak(Box::new(remap_config));

    let mut remaps = HashMap::new();
    boxed_config.remaps.iter().for_each(|r| {
        remaps.insert(
            (r.old_parent.as_str(), r.old_child.as_str()),
            (r.new_parent.as_str(), r.new_child.as_str()),
        );
    });

    let remaps1 = remaps.clone();

    let worker = node.create_worker::<usize>(0);

    let _subscription = worker.create_subscription::<TFMessage, _>(
        "/tf",
        move |msg: tf2_msgs::msg::TFMessage| {
            let mut new_msg = TFMessage::default();
            remap_transforms(&remaps, &msg.transforms, &mut new_msg.transforms);
            if new_msg.transforms.len() > 0 {
                let _ = publisher.publish(&new_msg);
            }
        },
    )?;

    let _static_subscription = worker.create_subscription::<TFMessage, _>(
        "/tf_static".transient_local(),
        move |msg: tf2_msgs::msg::TFMessage| {
            let mut new_msg = TFMessage::default();
            remap_transforms(&remaps1, &msg.transforms, &mut new_msg.transforms);
            if new_msg.transforms.len() > 0 {
                let _ = static_publisher.publish(&new_msg);
            }
        },
    )?;

    println!("Waiting for messages...");
    executor.spin(SpinOptions::default()).first_error()?;
    Ok(())
}

fn remap_transforms(
    remaps: &HashMap<(&str, &str), (&str, &str)>,
    transforms: &Vec<TransformStamped>,
    new_transforms: &mut Vec<TransformStamped>,
) {
    transforms.iter().for_each(|tf| {
        let parent = tf.header.frame_id.as_str();
        let child = tf.child_frame_id.as_str();
        let new_frames = remaps.get(&(parent, child)).copied();

        if let Some((new_parent, new_child)) = new_frames {
            let mut new_tf = tf.clone();
            new_tf.header.frame_id = new_parent.to_string();
            new_tf.child_frame_id = new_child.to_string();
            new_transforms.push(new_tf);
        }
    })
}
