"""macOS login Keychain storage; secrets never enter command arguments or files.

Uses the system Security framework directly. The legacy generic-password API is
available on macOS; Linux callers must explicitly use environment/ephemeral input.
"""
import ctypes as C
import sys

SERVICE = b's1code.credentials.v1'
PROVIDERS = ('claude', 'typesafe', 'openrouter')


class Keychain:
    def __init__(self, service=SERVICE):
        if sys.platform != 'darwin':
            raise RuntimeError('Keychain storage is only available on macOS; use --no-keychain.')
        self.service = service
        self.security = C.CDLL('/System/Library/Frameworks/Security.framework/Security')
        self.cf = C.CDLL('/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation')
        ptr, uint = C.c_void_p, C.c_uint32
        for name, args in {
            'SecKeychainFindGenericPassword': [ptr, uint, C.c_char_p, uint, C.c_char_p, C.POINTER(uint), C.POINTER(ptr), C.POINTER(ptr)],
            'SecKeychainAddGenericPassword': [ptr, uint, C.c_char_p, uint, C.c_char_p, uint, ptr, C.POINTER(ptr)],
            'SecKeychainItemModifyAttributesAndData': [ptr, ptr, uint, ptr],
            'SecKeychainItemFreeContent': [ptr, ptr],
            'SecKeychainItemDelete': [ptr],
        }.items():
            function = getattr(self.security, name)
            function.argtypes, function.restype = args, C.c_int32
        self.cf.CFRelease.argtypes, self.cf.CFRelease.restype = [ptr], None

    @staticmethod
    def checked(status):
        if status:
            raise RuntimeError(f'macOS Keychain operation failed (OSStatus {status}). Unlock/allow access in Keychain Access, or explicitly use --no-keychain. No plaintext fallback was used.')

    def find(self, provider, read=True):
        if provider not in PROVIDERS:
            raise ValueError('Unknown credential provider')
        account = provider.encode()
        length, data, item = C.c_uint32(), C.c_void_p(), C.c_void_p()
        status = self.security.SecKeychainFindGenericPassword(None, len(self.service), self.service,
            len(account), account, C.byref(length) if read else None,
            C.byref(data) if read else None, C.byref(item))
        if status == -25300:  # errSecItemNotFound
            return None, None
        self.checked(status)
        try:
            value = C.string_at(data, length.value).decode('utf-8') if read else None
        except Exception:
            self.cf.CFRelease(item)
            raise RuntimeError('Stored credential could not be decoded; replace it in Keychain Access.') from None
        finally:
            if data.value:
                self.security.SecKeychainItemFreeContent(None, data)
        return value, item

    def get(self, provider):
        value, item = self.find(provider)
        if item:
            self.cf.CFRelease(item)
        return value

    def set(self, provider, value):
        _, item = self.find(provider, read=False)
        data, account = value.encode(), provider.encode()
        try:
            if item:
                status = self.security.SecKeychainItemModifyAttributesAndData(item, None, len(data), data)
            else:
                status = self.security.SecKeychainAddGenericPassword(None, len(self.service), self.service,
                    len(account), account, len(data), data, None)
            self.checked(status)
        finally:
            if item:
                self.cf.CFRelease(item)

    def delete(self, provider):
        _, item = self.find(provider, read=False)
        if item:
            try:
                self.checked(self.security.SecKeychainItemDelete(item))
            finally:
                self.cf.CFRelease(item)
