//! The `settings!` macro: the single declaration every setting comes from.

/// Picks the first argument if present, else the fallback (`__settings_or!(; d)` → `d`).
#[doc(hidden)]
#[macro_export]
macro_rules! __settings_or {
    (; $fallback:expr) => {
        $fallback
    };
    ($value:expr; $fallback:expr) => {
        $value
    };
}

/// Declares the settings schema.
///
/// ```text
/// settings! {
///     <section>: <Struct> {
///         <name>: <Type> = <default> [, kind = <Kind>] [, restart = <bool>] [, stored = <bool>];
///         …
///     }
///     …
/// }
/// ```
///
/// It generates one struct per section, the [`Settings`](crate::Settings) struct with a field
/// per section, the [`Section`](crate::Section) enum, the [`SettingMeta`](crate::SettingMeta)
/// table, and typed get/set by key (`"viewing.initial_zoom"`). Without `kind`, the kind comes
/// from the type ([`SettingValue::default_kind`](crate::SettingValue::default_kind)). Message
/// ids are derived from the names: `settings-<section>-<name>` (label), `…-help`, and
/// `…-<option>` for choices, with `_` written as `-`.
#[macro_export]
macro_rules! settings {
    (
        $(
            $section:ident : $Struct:ident {
                $(
                    $name:ident : $ty:ty = $default:expr
                    $(, kind = $kind:expr)?
                    $(, restart = $restart:expr)?
                    $(, stored = $stored:expr)?
                    $(, os = $os:expr)?
                    ;
                )*
            }
        )*
    ) => {
        $(
            #[doc = concat!("The `[", stringify!($section), "]` section of `settings.toml`.")]
            #[derive(Debug, Clone, PartialEq)]
            pub struct $Struct {
                $( pub $name: $ty, )*
            }

            impl Default for $Struct {
                fn default() -> Self {
                    Self { $( $name: $default, )* }
                }
            }
        )*

        /// All settings, typed. Cheap to clone; the store hands out `Arc<Settings>` snapshots.
        #[derive(Debug, Clone, PartialEq, Default)]
        pub struct Settings {
            $( pub $section: $Struct, )*
        }

        /// A section of the Settings window and a table of `settings.toml`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Section {
            $( $Struct, )*
        }

        impl Section {
            /// All sections in window order.
            pub const ALL: &'static [Section] = &[ $( Section::$Struct, )* ];

            /// The TOML table name (`"viewing"`).
            pub fn key(self) -> &'static str {
                match self {
                    $( Section::$Struct => stringify!($section), )*
                }
            }

            /// Looks a section up by its TOML table name.
            pub fn from_key(key: &str) -> Option<Section> {
                Section::ALL.iter().copied().find(|s| s.key() == key)
            }
        }

        pub(crate) fn build_metas() -> Vec<$crate::SettingMeta> {
            #[allow(unused_mut)]
            let mut metas = Vec::new();
            $(
                #[allow(unused_variables)]
                let defaults = <$Struct as Default>::default();
                $(
                    metas.push($crate::SettingMeta::new(
                        Section::$Struct,
                        stringify!($section),
                        stringify!($name),
                        $crate::__settings_or!($($kind)?; <$ty as $crate::SettingValue>::default_kind()),
                        <$ty as $crate::SettingValue>::to_value(&defaults.$name),
                        $crate::__settings_or!($($restart)?; false),
                        $crate::__settings_or!($($stored)?; true),
                        $crate::__settings_or!($($os)?; &[]),
                    ));
                )*
            )*
            metas
        }

        impl Settings {
            /// The value of `key` (`"viewing.initial_zoom"`), or `None` if there is no such key.
            pub fn get(&self, key: &str) -> Option<$crate::Value> {
                $( $(
                    if key == concat!(stringify!($section), ".", stringify!($name)) {
                        return Some($crate::SettingValue::to_value(&self.$section.$name));
                    }
                )* )*
                None
            }

            /// Stores an already validated value. Returns whether the setting changed.
            pub(crate) fn put(&mut self, key: &str, value: &$crate::Value) -> bool {
                $( $(
                    if key == concat!(stringify!($section), ".", stringify!($name)) {
                        let Some(new) = <$ty as $crate::SettingValue>::from_value(value) else {
                            $crate::__log::error!("setting {key}: value {value:?} doesn't fit its type");
                            return false;
                        };
                        if self.$section.$name == new {
                            return false;
                        }
                        self.$section.$name = new;
                        return true;
                    }
                )* )*
                false
            }
        }
    };
}
