/// Generates code only compiled when the target pointer width is 64 bits.
///
/// # Example
///
/// ```rust
/// use orengine_utils::{config_target_pointer_width_16, config_target_pointer_width_32, config_target_pointer_width_64};
///
/// config_target_pointer_width_64! {
///     const VALUE: usize = 64;
/// }
///
/// config_target_pointer_width_32! {
///     const VALUE: usize = 32;
/// }
///
/// config_target_pointer_width_16! {
///     const VALUE: usize = 16;
/// }
///
/// #[cfg(target_pointer_width = "64")]
/// const SHOULD_BE: usize = 64;
///
/// #[cfg(target_pointer_width = "32")]
/// const SHOULD_BE: usize = 32;
///
/// #[cfg(target_pointer_width = "16")]
/// const SHOULD_BE: usize = 16;
///
/// assert_eq!(VALUE, SHOULD_BE);
/// ```
#[macro_export]
macro_rules! config_target_pointer_width_64 {
($($item:item)*) => {
    $(
        #[cfg(target_pointer_width = "64")]
        $item
    )*
};
}

/// Generates code only compiled when the target pointer width is 32 bits.
///
/// # Example
///
/// ```rust
/// use orengine_utils::{config_target_pointer_width_16, config_target_pointer_width_32, config_target_pointer_width_64};
///
/// config_target_pointer_width_64! {
///     const VALUE: usize = 64;
/// }
///
/// config_target_pointer_width_32! {
///     const VALUE: usize = 32;
/// }
///
/// config_target_pointer_width_16! {
///     const VALUE: usize = 16;
/// }
///
/// #[cfg(target_pointer_width = "64")]
/// const SHOULD_BE: usize = 64;
///
/// #[cfg(target_pointer_width = "32")]
/// const SHOULD_BE: usize = 32;
///
/// #[cfg(target_pointer_width = "16")]
/// const SHOULD_BE: usize = 16;
///
/// assert_eq!(VALUE, SHOULD_BE);
/// ```
#[macro_export]
macro_rules! config_target_pointer_width_32 {
($($item:item)*) => {
    $(
        #[cfg(target_pointer_width = "32")]
        $item
    )*
};
}

/// Generates code only compiled when the target pointer width is 16 bits.
///
/// # Example
///
/// ```rust
/// use orengine_utils::{config_target_pointer_width_16, config_target_pointer_width_32, config_target_pointer_width_64};
///
/// config_target_pointer_width_64! {
///     const VALUE: usize = 64;
/// }
///
/// config_target_pointer_width_32! {
///     const VALUE: usize = 32;
/// }
///
/// config_target_pointer_width_16! {
///     const VALUE: usize = 16;
/// }
///
/// #[cfg(target_pointer_width = "64")]
/// const SHOULD_BE: usize = 64;
///
/// #[cfg(target_pointer_width = "32")]
/// const SHOULD_BE: usize = 32;
///
/// #[cfg(target_pointer_width = "16")]
/// const SHOULD_BE: usize = 16;
///
/// assert_eq!(VALUE, SHOULD_BE);
/// ```
#[macro_export]
macro_rules! config_target_pointer_width_16 {
($($item:item)*) => {
    $(
        #[cfg(target_pointer_width = "16")]
        $item
    )*
};
}
