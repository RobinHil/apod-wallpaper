//! Handing back what an update borrowed.
//!
//! An update is the only moment this application allocates anything large: a
//! full-resolution APOD decoded into memory, then two screen-sized buffers to
//! compose it over. All of it is dropped as soon as the wallpaper is on disk
//! -- but dropped is not returned. The allocator keeps the arena, and the
//! process goes on to sleep until the next day change holding tens of
//! megabytes it will not read again for twelve hours.
//!
//! Measured on GNOME with a 2.2-megapixel APOD: 13.1 MB of private dirty
//! memory on a start with nothing to do, 25.7 MB after one update, for a
//! wallpaper that was finished, flushed and applied. A 29-megapixel original,
//! which APOD publishes regularly, costs proportionally more.
//!
//! This is the whole of the fix, and it is deliberately not clever: no arena
//! tuning, no allocator swapped in, no pool. One call, at the one moment when
//! everything large has certainly been dropped.

/// Returns what the last update freed to the operating system.
///
/// Called once at the end of `updater::update`, which is where every large
/// buffer has been dropped and the process is about to go back to sleep.
/// Cheap enough to call on the paths that allocated nothing: it walks the
/// allocator's free lists, finds nothing worth returning, and stops.
pub fn release_to_os() {
    // glibc only, and that is not an oversight.
    //
    // musl returns memory to the kernel far more eagerly and exposes no
    // equivalent knob. macOS has no `malloc_trim` either; its allocator has
    // `malloc_zone_pressure_relief`, which `libc` does not bind and which is
    // not worth declaring by hand for a saving this size. Both simply keep
    // what they keep, which is the behaviour this application had everywhere
    // until now.
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    // SAFETY: `malloc_trim` takes a byte count to leave at the top of the
    // heap and touches nothing the program holds a reference to. It is safe
    // to call at any time and from any thread; the return value only says
    // whether it managed to release anything, which is not worth acting on.
    unsafe {
        libc::malloc_trim(0);
    }
}
