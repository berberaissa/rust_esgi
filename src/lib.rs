/// Total heap size in bytes.
pub const HEAP_SIZE: usize = 1024;

/// Size of one slot in bytes.
pub const SLOT_SIZE: usize = 128;

/// Returns the number of slots in the allocator.
///
/// # Examples
///
/// ```
/// use rust_esgi::num_slots;
///
/// let slots = num_slots();
/// assert_eq!(slots, 8);
/// ```
pub fn num_slots() -> usize {
    HEAP_SIZE / SLOT_SIZE
}

/// Returns true if a requested size fits in one slot.
///
/// # Examples
///
/// ```
/// use rust_esgi::fits_in_slot;
///
/// assert!(fits_in_slot(64));
/// assert!(fits_in_slot(128));
/// assert!(!fits_in_slot(129));
/// ```
pub fn fits_in_slot(size: usize) -> bool {
    size <= SLOT_SIZE
}