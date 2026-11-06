use std::ffi::{CStr, CString, c_int, c_void};
use std::fmt;
use std::mem::size_of;
use std::ptr;

use libpam_sys::{
    PAM_ACCT_EXPIRED, PAM_AUTH_ERR, PAM_AUTHTOK_EXPIRED, PAM_BUF_ERR, PAM_CONV_ERR,
    PAM_CRED_EXPIRED, PAM_CRED_INSUFFICIENT, PAM_ERROR_MSG, PAM_ESTABLISH_CRED, PAM_MAXTRIES,
    PAM_NEW_AUTHTOK_REQD, PAM_PERM_DENIED, PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON,
    PAM_REINITIALIZE_CRED, PAM_SUCCESS, PAM_SYSTEM_ERR, PAM_TEXT_INFO, PAM_TTY, PAM_USER_UNKNOWN,
    pam_acct_mgmt, pam_authenticate, pam_close_session, pam_conv, pam_end, pam_getenvlist,
    pam_handle, pam_message, pam_open_session, pam_response, pam_set_item, pam_setcred, pam_start,
    pam_strerror,
};

const PAM_SERVICE: &str = "ulm";

struct ConvData {
    user: CString,
    password: CString,
}

pub struct PamSession {
    handle: *mut pam_handle,
    _conv: Box<pam_conv>,
    _data: Box<ConvData>,
    last_code: c_int,
    session_open: bool,
}

pub enum AuthResult {
    Success(PamSession),
    Invalid,
    Unavailable,
}

#[derive(Debug)]
pub struct PamError {
    code: c_int,
    message: String,
}

impl PamError {
    fn new(handle: *const pam_handle, code: c_int) -> Self {
        let message = unsafe {
            let ptr = pam_strerror(handle, code);
            if ptr.is_null() {
                format!("PAM error {code}")
            } else {
                CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };

        Self { code, message }
    }

    fn invalid(message: &str) -> Self {
        Self {
            code: PAM_SYSTEM_ERR,
            message: message.to_string(),
        }
    }

    pub fn code(&self) -> c_int {
        self.code
    }
}

impl fmt::Display for PamError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (PAM {})", self.message, self.code)
    }
}

impl std::error::Error for PamError {}

impl PamSession {
    fn start(user: &str, password: &str) -> Result<Self, PamError> {
        let service =
            CString::new(PAM_SERVICE).map_err(|_| PamError::invalid("invalid PAM service"))?;
        let c_user = CString::new(user).map_err(|_| PamError::invalid("invalid username"))?;
        let c_password =
            CString::new(password).map_err(|_| PamError::invalid("invalid password"))?;

        let data = Box::new(ConvData {
            user: c_user.clone(),
            password: c_password,
        });
        let mut conv = Box::new(pam_conv {
            conv: conversation,
            appdata_ptr: (&*data as *const ConvData as *mut ConvData).cast(),
        });

        let mut handle = ptr::null_mut();
        let rc = unsafe { pam_start(service.as_ptr(), c_user.as_ptr(), &mut *conv, &mut handle) };
        if rc != PAM_SUCCESS {
            return Err(PamError::new(handle, rc));
        }

        let session = Self {
            handle,
            _conv: conv,
            _data: data,
            last_code: rc,
            session_open: false,
        };
        session.set_tty();
        Ok(session)
    }

    fn set_tty(&self) {
        unsafe {
            let tty = libc::ttyname(0);
            if tty.is_null() {
                return;
            }
            let tty = CStr::from_ptr(tty).to_owned();
            let _ = pam_set_item(self.handle, PAM_TTY, tty.as_ptr().cast());
        }
    }

    fn authenticate(&mut self) -> Result<(), PamError> {
        let rc = unsafe { pam_authenticate(self.handle, 0) };
        self.check(rc)
    }

    fn account_management(&mut self) -> Result<(), PamError> {
        let rc = unsafe { pam_acct_mgmt(self.handle, 0) };
        self.check(rc)
    }

    fn setcred(&mut self, flags: c_int) -> Result<(), PamError> {
        let rc = unsafe { pam_setcred(self.handle, flags) };
        self.check(rc)
    }

    pub fn open(&mut self) -> Result<(), PamError> {
        self.setcred(PAM_ESTABLISH_CRED)?;

        let rc = unsafe { pam_open_session(self.handle, 0) };
        self.check(rc)?;
        self.session_open = true;

        self.setcred(PAM_REINITIALIZE_CRED)?;
        self.apply_env()
    }

    fn apply_env(&self) -> Result<(), PamError> {
        let list = unsafe { pam_getenvlist(self.handle) };
        if list.is_null() {
            return Ok(());
        }

        let mut index = 0;
        loop {
            let entry = unsafe { *list.add(index) };
            if entry.is_null() {
                break;
            }

            let bytes = unsafe { CStr::from_ptr(entry) }.to_bytes();
            if let Some(eq) = bytes.iter().position(|&byte| byte == b'=')
                && let (Ok(name), Ok(value)) =
                    (CString::new(&bytes[..eq]), CString::new(&bytes[eq + 1..]))
            {
                unsafe {
                    libc::setenv(name.as_ptr(), value.as_ptr(), 1);
                }
            }

            unsafe {
                libc::free(entry.cast());
            }
            index += 1;
        }

        unsafe {
            libc::free(list.cast());
        }
        Ok(())
    }

    fn check(&mut self, rc: c_int) -> Result<(), PamError> {
        self.last_code = rc;
        if rc == PAM_SUCCESS {
            Ok(())
        } else {
            Err(PamError::new(self.handle, rc))
        }
    }
}

impl Drop for PamSession {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }

        unsafe {
            if self.session_open {
                pam_close_session(self.handle, 0);
            }
            pam_end(self.handle, self.last_code);
        }
    }
}

pub fn authenticate(user: &str, password: &str) -> AuthResult {
    let mut session = match PamSession::start(user, password) {
        Ok(session) => session,
        Err(_) => return AuthResult::Unavailable,
    };

    if let Err(err) = session.authenticate() {
        return map_error(err.code());
    }

    match session.account_management() {
        Ok(()) => AuthResult::Success(session),
        Err(err) => map_error(err.code()),
    }
}

fn map_error(code: c_int) -> AuthResult {
    match code {
        PAM_AUTH_ERR
        | PAM_CRED_INSUFFICIENT
        | PAM_USER_UNKNOWN
        | PAM_MAXTRIES
        | PAM_NEW_AUTHTOK_REQD
        | PAM_ACCT_EXPIRED
        | PAM_CRED_EXPIRED
        | PAM_AUTHTOK_EXPIRED
        | PAM_PERM_DENIED => AuthResult::Invalid,
        _ => AuthResult::Unavailable,
    }
}

unsafe extern "C" fn conversation(
    num_msg: c_int,
    msg: *const *const pam_message,
    resp: *mut *mut pam_response,
    appdata: *mut c_void,
) -> c_int {
    if appdata.is_null() || msg.is_null() || resp.is_null() || num_msg <= 0 {
        return PAM_CONV_ERR;
    }

    unsafe {
        *resp = ptr::null_mut();
    }

    let data = unsafe { &*(appdata as *const ConvData) };
    let count = num_msg as usize;
    let responses = unsafe { libc::calloc(count, size_of::<pam_response>()) as *mut pam_response };
    if responses.is_null() {
        return PAM_BUF_ERR;
    }

    for index in 0..count {
        let message = unsafe { &**msg.add(index) };
        let response = unsafe { &mut *responses.add(index) };
        response.resp = ptr::null_mut();
        response.resp_retcode = 0;

        match message.msg_style {
            PAM_PROMPT_ECHO_ON => {
                response.resp = unsafe { libc::strdup(data.user.as_ptr()) };
            }
            PAM_PROMPT_ECHO_OFF => {
                response.resp = unsafe { libc::strdup(data.password.as_ptr()) };
            }
            PAM_ERROR_MSG | PAM_TEXT_INFO => {}
            _ => {
                unsafe {
                    free_responses(responses, count);
                }
                return PAM_CONV_ERR;
            }
        }

        if matches!(message.msg_style, PAM_PROMPT_ECHO_ON | PAM_PROMPT_ECHO_OFF)
            && response.resp.is_null()
        {
            unsafe {
                free_responses(responses, count);
            }
            return PAM_BUF_ERR;
        }
    }

    unsafe {
        *resp = responses;
    }
    PAM_SUCCESS
}

unsafe fn free_responses(responses: *mut pam_response, count: usize) {
    if responses.is_null() {
        return;
    }

    for index in 0..count {
        let response = unsafe { &mut *responses.add(index) };
        if !response.resp.is_null() {
            unsafe {
                libc::free(response.resp.cast());
            }
        }
    }

    unsafe {
        libc::free(responses.cast());
    }
}
