//! Numbers: what an app knows registered things by, as Bevy knows components
//! by `ComponentId`.

/// Defines a number type for something an app registers: dense, given in the
/// order things are added, and what state, data and the network know it by.
///
/// As Bevy's `ComponentId`: its name lives beside it, in its registry, for
/// data, debugging and what's shown.
///
/// ```
/// bevy_mettle_core::number! {
///     /// A sword's number.
///     pub struct SwordId;
/// }
///
/// let sword = SwordId::new(3);
/// assert_eq!(sword.index(), 3);
/// ```
#[macro_export]
macro_rules! number {
    ($(#[$meta:meta])* $vis:vis struct $name:ident;) => {
        $(#[$meta])*
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            $crate::__private::Reflect
        )]
        #[reflect(Debug, Clone, PartialEq, Hash)]
        $vis struct $name(u32);

        /// Written as its number, a newtype.
        impl $crate::__private::serde::Serialize for $name {
            fn serialize<S: $crate::__private::serde::Serializer>(
                &self,
                serializer: S,
            ) -> ::core::result::Result<S::Ok, S::Error> {
                serializer.serialize_newtype_struct(::core::stringify!($name), &self.0)
            }
        }

        /// Read as its number, a newtype.
        impl<'de> $crate::__private::serde::Deserialize<'de> for $name {
            fn deserialize<D: $crate::__private::serde::Deserializer<'de>>(
                deserializer: D,
            ) -> ::core::result::Result<Self, D::Error> {
                struct Number;

                impl<'de> $crate::__private::serde::de::Visitor<'de> for Number {
                    type Value = $name;

                    fn expecting(
                        &self,
                        formatter: &mut ::core::fmt::Formatter<'_>,
                    ) -> ::core::fmt::Result {
                        formatter.write_str(::core::concat!(
                            "a ",
                            ::core::stringify!($name),
                            ": a number"
                        ))
                    }

                    fn visit_newtype_struct<D: $crate::__private::serde::Deserializer<'de>>(
                        self,
                        deserializer: D,
                    ) -> ::core::result::Result<$name, D::Error> {
                        <u32 as $crate::__private::serde::Deserialize>::deserialize(deserializer)
                            .map($name)
                    }
                }

                deserializer.deserialize_newtype_struct(::core::stringify!($name), Number)
            }
        }

        impl $name {
            /// The one numbered `index`.
            #[must_use]
            pub const fn new(index: u32) -> Self {
                Self(index)
            }

            /// Its index: where it's kept, in its registry or an actor's
            /// state.
            #[must_use]
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }

        impl $crate::Number for $name {
            fn from_index(index: u32) -> Self {
                Self(index)
            }

            fn to_index(self) -> usize {
                self.0 as usize
            }
        }
    };
}
