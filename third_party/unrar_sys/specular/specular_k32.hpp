// Specular (specular-no-win32k.patch): kernel32 stand-ins for the user32 calls UnRAR makes
// (case mapping, and OEM <-> ANSI conversion of names and comments).
//
// The sandboxed decode host disables Win32k system calls once it has started, which Windows
// refuses (ERROR_WRITE_PROTECT) for a process that has loaded user32.dll; and a call into
// user32 after that point would end the process. Included by os.hpp after every Windows
// header, so these macros rename the calls without touching the declarations they replace.

#ifndef SPECULAR_K32_HPP
#define SPECULAR_K32_HPP

#include <windows.h>

LPWSTR WINAPI SpecularCharLowerW(LPWSTR s);
LPWSTR WINAPI SpecularCharUpperW(LPWSTR s);
BOOL WINAPI SpecularOemToCharA(LPCSTR src, LPSTR dst);
BOOL WINAPI SpecularOemToCharBuffA(LPCSTR src, LPSTR dst, DWORD len);
BOOL WINAPI SpecularCharToOemA(LPCSTR src, LPSTR dst);
BOOL WINAPI SpecularCharToOemBuffW(LPCWSTR src, LPSTR dst, DWORD len);

// CharLower and CharUpper expand to the W names (UNICODE), and those to these.
#define CharLowerW SpecularCharLowerW
#define CharUpperW SpecularCharUpperW
#define OemToCharA SpecularOemToCharA
#define OemToCharBuffA SpecularOemToCharBuffA
#define CharToOemA SpecularCharToOemA
#define CharToOemBuffW SpecularCharToOemBuffW

#endif
