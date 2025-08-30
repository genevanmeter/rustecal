//! # rustecal-types-protobuf
//!
//! Provides support for Protobuf message serialization with rustecal.

use prost::Message;
use prost_reflect::{FileDescriptor, ReflectMessage};
use rustecal_core::types::DataTypeInfo;
use rustecal_pubsub::typed_publisher::PublisherMessage;
use rustecal_pubsub::typed_subscriber::SubscriberMessage;
use std::sync::Arc;

/// Marker trait to opt-in a Protobuf type for use with eCAL.
///
/// This trait must be implemented for any `prost::Message` you wish to use
/// with `ProtobufMessage<T>`. It provides a type-level opt-in mechanism
/// to ensure users are aware of what's being exposed to eCAL.
pub trait IsProtobufType {}

/// A wrapper for protobuf messages used with typed eCAL pub/sub.
///
/// This type allows sending and receiving protobuf messages through the
/// `TypedPublisher` and `TypedSubscriber` APIs.
#[derive(Debug, Clone)]
pub struct ProtobufMessage<T> {
    pub data: Arc<T>,
}

impl<T> SubscriberMessage<'_> for ProtobufMessage<T>
where
    T: Message + Default + IsProtobufType + ReflectMessage,
{
    /// Returns metadata used by eCAL to describe the Protobuf type.
    ///
    /// This includes:
    /// - `proto` as encoding
    /// - the Rust type name
    /// - an optional descriptor
    fn datatype() -> DataTypeInfo {
        let default_instance = T::default();
        let instance_descriptor = default_instance.descriptor();
        let type_name = instance_descriptor.full_name().to_string();

        let mut descriptor_pool = prost_reflect::DescriptorPool::new();

        // List of proto files for a specific protobuf message type
        let mut proto_filenames = instance_descriptor
            .parent_file_descriptor_proto()
            .dependency
            .clone();
        // Add the main proto message file
        proto_filenames.push(
            instance_descriptor
                .parent_file_descriptor_proto()
                .name()
                .to_string(),
        );

        // Filter the pool to the set of file decriptors needed
        let file_descriptors: Vec<FileDescriptor> = instance_descriptor
            .parent_pool()
            .files()
            .filter(|s| proto_filenames.contains(&s.name().to_string()))
            .collect();

        for proto_file in file_descriptors.iter() {
            let mut file_descriptor_proto = proto_file.file_descriptor_proto().clone();
            // Remove the source_code_info from the descriptor which add excess comments
            // from original proto to the descriptor message that aren't needed
            file_descriptor_proto.source_code_info = None;

            descriptor_pool
                .add_file_descriptor_proto(file_descriptor_proto)
                .unwrap();
        }

        DataTypeInfo {
            encoding: "proto".to_string(),
            type_name,
            descriptor: descriptor_pool.encode_to_vec(),
        }
    }

    /// Decodes a Protobuf message from bytes.
    ///
    /// # Returns
    /// - `Some(ProtobufMessage<T>)` on success
    /// - `None` if decoding fails
    fn from_bytes(bytes: &[u8], _data_type_info: &DataTypeInfo) -> Option<Self> {
        T::decode(bytes).ok().map(|msg| ProtobufMessage {
            data: Arc::new(msg),
        })
    }
}

impl<T> PublisherMessage for ProtobufMessage<T>
where
    T: Message + Default + IsProtobufType + ReflectMessage,
{
    /// Returns the same datatype information as [`SubscriberMessage`]
    /// implementation.
    fn datatype() -> DataTypeInfo {
        <ProtobufMessage<T> as SubscriberMessage>::datatype()
    }

    /// Encodes the message to a byte buffer.
    ///
    /// # Panics
    /// Will panic if `prost::Message::encode` fails (should never panic for
    /// valid messages).
    fn to_bytes(&self) -> Arc<[u8]> {
        let mut buf = Vec::with_capacity(self.data.encoded_len());
        self.data
            .encode(&mut buf)
            .expect("Failed to encode protobuf message");
        Arc::from(buf)
    }
}
