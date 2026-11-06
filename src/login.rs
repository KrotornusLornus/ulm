use std::ffi::{CStr, CString, OsStr, OsString, c_char};
use std::io;
use std::mem::zeroed;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
use std::ptr;

const DEFAULT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
const DEFAULT_SHELL: &str = "/bin/sh";

pub fn exec_shell(user: &str) -> io::Result<()> {
    let c_user = CString::new(user).map_err(|_| invalid("invalid username"))?;

    let pw = unsafe { libc::getpwnam(c_user.as_ptr()) };
    if pw.is_null() {
        return Err(io::Error::new(io::ErrorKind::NotFound, "unknown user"));
    }

    let pw = unsafe { &*pw };
    let name = unsafe { os_string(pw.pw_name, "name")? };
    let home = unsafe { os_string(pw.pw_dir, "home")? };
    let shell = unsafe { os_string(pw.pw_shell, "shell")? };
    let uid = pw.pw_uid;
    let gid = pw.pw_gid;

    let c_name = CString::new(name.as_bytes()).map_err(|_| invalid("invalid name"))?;
    let c_home = CString::new(home.as_bytes()).map_err(|_| invalid("invalid home"))?;
    let shell = if shell.is_empty() {
        OsString::from(DEFAULT_SHELL)
    } else {
        shell
    };

    let tty = tty_name();
    if let Some(tty) = &tty {
        set_tty_owner(tty, uid, gid);
        record_utmp(&name, tty);
    }

    if unsafe { libc::chdir(c_home.as_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }

    if unsafe { libc::initgroups(c_name.as_ptr(), gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::setgid(gid) } != 0 {
        return Err(io::Error::last_os_error());
    }
    if unsafe { libc::setuid(uid) } != 0 {
        return Err(io::Error::last_os_error());
    }

    set_env("HOME", &home)?;
    set_env("USER", &name)?;
    set_env("LOGNAME", &name)?;
    set_env("SHELL", &shell)?;
    set_env("PATH", OsStr::new(DEFAULT_PATH))?;
    set_env("PWD", &home)?;
    if std::env::var_os("TERM").is_none() {
        set_env("TERM", OsStr::new("linux"))?;
    }

    let shell_name = Path::new(&shell)
        .file_name()
        .unwrap_or_else(|| OsStr::new("sh"));
    let argv0 = format!("-{}", shell_name.to_string_lossy());

    Err(Command::new(&shell).arg0(argv0).exec())
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

unsafe fn os_string(ptr: *const c_char, field: &str) -> io::Result<OsString> {
    if ptr.is_null() {
        return Err(invalid(&format!("missing passwd {field}")));
    }

    Ok(OsString::from_vec(
        unsafe { CStr::from_ptr(ptr) }.to_bytes().to_vec(),
    ))
}

fn tty_name() -> Option<CString> {
    unsafe {
        let tty = libc::ttyname(0);
        if tty.is_null() {
            None
        } else {
            Some(CStr::from_ptr(tty).to_owned())
        }
    }
}

fn set_tty_owner(tty: &CStr, uid: libc::uid_t, gid: libc::gid_t) {
    unsafe {
        libc::chown(tty.as_ptr(), uid, gid);
        libc::chmod(tty.as_ptr(), 0o620);
    }
}

fn record_utmp(name: &OsString, tty: &CString) {
    let line = tty.as_bytes();
    let line = line.strip_prefix(b"/dev/").unwrap_or(line);

    unsafe {
        let mut ut: libc::utmpx = zeroed();
        ut.ut_type = libc::USER_PROCESS;
        ut.ut_pid = libc::getpid();
        copy_cstr(&mut ut.ut_line, line);
        copy_raw(&mut ut.ut_id, line);
        copy_cstr(&mut ut.ut_user, name.as_bytes());
        ut.ut_session = libc::getsid(0) as _;

        let mut tv: libc::timeval = zeroed();
        libc::gettimeofday(&mut tv, ptr::null_mut());
        ut.ut_tv.tv_sec = tv.tv_sec as i32;
        ut.ut_tv.tv_usec = tv.tv_usec as i32;

        libc::setutxent();
        libc::pututxline(&ut);
        libc::endutxent();
    }
}

fn copy_cstr<const N: usize>(dst: &mut [c_char; N], src: &[u8]) {
    let len = src.len().min(N.saturating_sub(1));
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr().cast(), dst.as_mut_ptr(), len);
    }
}

fn copy_raw<const N: usize>(dst: &mut [c_char; N], src: &[u8]) {
    let len = src.len().min(N);
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr().cast(), dst.as_mut_ptr(), len);
    }
}

fn set_env(name: &str, value: &OsStr) -> io::Result<()> {
    let name = CString::new(name).map_err(|_| invalid("invalid env name"))?;
    let value = CString::new(value.as_bytes()).map_err(|_| invalid("invalid env value"))?;

    if unsafe { libc::setenv(name.as_ptr(), value.as_ptr(), 1) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
