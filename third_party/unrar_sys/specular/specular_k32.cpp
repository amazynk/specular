// Specular (specular-no-win32k.patch): kernel32 versions of the user32 calls in
// specular_k32.hpp, with the same results for UnRAR's uses.

#define WIN32_LEAN_AND_MEAN
#include <windows.h>

#include <string>

#include "specular_k32.hpp"

// CharLowerW/CharUpperW: a string in place, or a single character in the low word of the
// "pointer" (its high word zero), returned the same way. LCMapStringW maps by the user's
// locale, as user32 does.
static LPWSTR MapCase(LPWSTR s, DWORD flag)
{
  if (((ULONG_PTR)s >> 16) == 0)
  {
    WCHAR c = (WCHAR)(ULONG_PTR)s, r = c;
    if (LCMapStringW(LOCALE_USER_DEFAULT, flag, &c, 1, &r, 1) != 1)
      r = c;
    return (LPWSTR)(ULONG_PTR)r;
  }
  int n = lstrlenW(s);
  if (n > 0)
  {
    std::wstring src(s, (size_t)n);
    if (LCMapStringW(LOCALE_USER_DEFAULT, flag, src.data(), n, s, n) != n)
      wmemcpy(s, src.data(), (size_t)n);
  }
  return s;
}

LPWSTR WINAPI SpecularCharLowerW(LPWSTR s)
{
  return MapCase(s, LCMAP_LOWERCASE);
}

LPWSTR WINAPI SpecularCharUpperW(LPWSTR s)
{
  return MapCase(s, LCMAP_UPPERCASE);
}

// `len` bytes of `src` from code page `from` to `to`, into `dst` (at most `len` bytes, as the
// user32 calls write). `src` and `dst` may be the same buffer: UnRAR converts in place.
static BOOL Recode(UINT from, UINT to, LPCSTR src, int len, LPSTR dst)
{
  if (len <= 0)
    return TRUE;
  int n = MultiByteToWideChar(from, 0, src, len, NULL, 0);
  if (n <= 0)
    return FALSE;
  std::wstring w((size_t)n, L'\0');
  MultiByteToWideChar(from, 0, src, len, &w[0], n);
  int m = WideCharToMultiByte(to, 0, w.data(), n, NULL, 0, NULL, NULL);
  if (m <= 0)
    return FALSE;
  std::string out((size_t)m, '\0');
  WideCharToMultiByte(to, 0, w.data(), n, &out[0], m, NULL, NULL);
  memcpy(dst, out.data(), (size_t)(m < len ? m : len));
  return TRUE;
}

// The NUL-terminated forms convert the terminator too, so the result stays terminated.
static BOOL RecodeString(UINT from, UINT to, LPCSTR src, LPSTR dst)
{
  int len = lstrlenA(src) + 1;
  BOOL ok = Recode(from, to, src, len, dst);
  dst[len - 1] = '\0';
  return ok;
}

BOOL WINAPI SpecularOemToCharA(LPCSTR src, LPSTR dst)
{
  return RecodeString(CP_OEMCP, CP_ACP, src, dst);
}

BOOL WINAPI SpecularOemToCharBuffA(LPCSTR src, LPSTR dst, DWORD len)
{
  return Recode(CP_OEMCP, CP_ACP, src, (int)len, dst);
}

BOOL WINAPI SpecularCharToOemA(LPCSTR src, LPSTR dst)
{
  return RecodeString(CP_ACP, CP_OEMCP, src, dst);
}

BOOL WINAPI SpecularCharToOemBuffW(LPCWSTR src, LPSTR dst, DWORD len)
{
  if (len == 0)
    return TRUE;
  int m = WideCharToMultiByte(CP_OEMCP, 0, src, (int)len, NULL, 0, NULL, NULL);
  if (m <= 0)
    return FALSE;
  std::string out((size_t)m, '\0');
  WideCharToMultiByte(CP_OEMCP, 0, src, (int)len, &out[0], m, NULL, NULL);
  memcpy(dst, out.data(), (size_t)(m < (int)len ? m : (int)len));
  return TRUE;
}
