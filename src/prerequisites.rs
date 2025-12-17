use std::ffi::CString;
use std::io;

/// Returns true if the current process is in the given group (by name).
/// This checks real/effective GID and the current supplementary group list.
/// Note: If a user was just added to a group, a new login session (or `newgrp`)
/// is needed for that membership to appear here.
pub fn is_current_user_in_group(group_name: &str) -> io::Result<bool> {
    // Map group name -> gid using getgrnam_r
    let c_name = CString::new(group_name)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "group name contains NUL"))?;

    // Get a reasonable buffer size for getgrnam_r
    let mut buf_len = unsafe { libc::sysconf(libc::_SC_GETGR_R_SIZE_MAX) };
    if buf_len < 0 {
        buf_len = 16 * 1024; // fallback if sysconf is unavailable
    }
    let mut buf = vec![0u8; buf_len as usize];
    let mut grp: libc::group = unsafe { std::mem::zeroed() };
    let mut result: *mut libc::group = std::ptr::null_mut();

    let rc = unsafe {
        libc::getgrnam_r(
            c_name.as_ptr(),
            &mut grp as *mut libc::group,
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            &mut result as *mut *mut libc::group,
        )
    };
    if rc != 0 {
        return Err(io::Error::from_raw_os_error(rc));
    }
    if result.is_null() {
        // group does not exist
        return Ok(false);
    }
    let target_gid = unsafe { (*result).gr_gid };

    // Fast path: check real/effective GID
    let rgid = unsafe { libc::getgid() };
    let egid = unsafe { libc::getegid() };
    if rgid == target_gid || egid == target_gid {
        return Ok(true);
    }

    // Check supplementary groups
    let ngroups = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if ngroups < 0 {
        return Err(io::Error::last_os_error());
    }
    if ngroups == 0 {
        return Ok(false);
    }

    let mut groups: Vec<libc::gid_t> = vec![0; ngroups as usize];
    let ngroups2 = unsafe { libc::getgroups(groups.len() as i32, groups.as_mut_ptr()) };
    if ngroups2 < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(groups[..(ngroups2 as usize)]
        .iter()
        .copied()
        .any(|g| g == target_gid))
}
