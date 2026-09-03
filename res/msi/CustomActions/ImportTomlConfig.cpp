// ImportTomlConfig.cpp : Import rustdesk-config-import.toml during MSI installation.
//
// The deferred custom action runs as SYSTEM, but the import writes the
// *logged-in user's* config (%APPDATA%\RustDesk\config). So for every active
// user session we grab the user's token (WTSQueryUserToken) and launch
// `RustDesk.exe --import-toml-config <toml>` as that user via CreateProcessAsUser.
//
// CustomActionData format (set by ImportTomlConfig.SetParam in RustDesk.wxs):
//   <msi full path>|<installed exe full path>|<toml file name>
// The toml file is looked up in the same directory as the MSI.
//
// This custom action never fails the installation: a missing toml, a missing
// user session, or a non-zero exit code is only logged.

#include "pch.h"
#include "./Common.h"
#include <strutil.h>

#include <wtsapi32.h>
#include <userenv.h>

#pragma comment(lib, "wtsapi32.lib")
#pragma comment(lib, "userenv.lib")

static const DWORD IMPORT_WAIT_TIMEOUT_MS = 60 * 1000;

static bool FileExistsW(LPCWSTR path)
{
    DWORD attributes = GetFileAttributesW(path);
    return attributes != INVALID_FILE_ATTRIBUTES && !(attributes & FILE_ATTRIBUTE_DIRECTORY);
}

// Run the import once with the given interactive-user token.
static void RunImportAsUser(HANDLE hUserToken, DWORD sessionId, LPCWSTR exePath, LPCWSTR tomlPath)
{
    LPVOID environment = NULL;
    STARTUPINFOW si = {};
    PROCESS_INFORMATION pi = {};
    WCHAR commandLine[2048] = { 0 };
    BOOL processCreated = FALSE;

    if (CreateEnvironmentBlock(&environment, hUserToken, FALSE)) {
        WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: environment block created for session %lu.", sessionId);
    }
    else {
        WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: CreateEnvironmentBlock failed for session %lu, error: %lu. Continue without it.", sessionId, GetLastError());
        environment = NULL;
    }

    si.cb = sizeof(si);
    si.lpDesktop = (LPWSTR)L"winsta0\\default";

    if (FAILED(StringCchPrintfW(commandLine, ARRAYSIZE(commandLine), L"\"%ls\" --import-toml-config \"%ls\"", exePath, tomlPath))) {
        WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: command line is too long, session %lu.", sessionId);
        goto LExit;
    }

    WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: session %lu, run as user: %ls", sessionId, commandLine);

    {
        DWORD creationFlags = (environment != NULL) ? (CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT) : CREATE_NO_WINDOW;
        processCreated = CreateProcessAsUserW(
            hUserToken,
            NULL,
            commandLine,
            NULL,
            NULL,
            FALSE,
            creationFlags,
            environment,
            NULL,
            &si,
            &pi);
    }

    if (!processCreated) {
        WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: CreateProcessAsUser failed for session %lu, error: %lu.", sessionId, GetLastError());
        goto LExit;
    }

    {
        DWORD waitResult = WaitForSingleObject(pi.hProcess, IMPORT_WAIT_TIMEOUT_MS);
        if (waitResult == WAIT_OBJECT_0) {
            DWORD exitCode = 0;
            if (GetExitCodeProcess(pi.hProcess, &exitCode)) {
                // 0 = success, see docs/toml-config-import.md for the full table.
                WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: import exited with code %lu for session %lu.", exitCode, sessionId);
            }
            else {
                WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: import finished but GetExitCodeProcess failed, error: %lu.", GetLastError());
            }
        }
        else if (waitResult == WAIT_TIMEOUT) {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: import did not exit within %lu ms for session %lu, continue installation.", IMPORT_WAIT_TIMEOUT_MS, sessionId);
        }
        else {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: WaitForSingleObject failed for session %lu, error: %lu.", sessionId, GetLastError());
        }
    }

LExit:
    if (pi.hProcess) {
        CloseHandle(pi.hProcess);
    }
    if (pi.hThread) {
        CloseHandle(pi.hThread);
    }
    if (environment != NULL) {
        DestroyEnvironmentBlock(environment);
    }
}

// See `Package.wxs` / `Components/RustDesk.wxs` for the sequence of this custom action.
//
// Install sequence (fresh install or upgrade, never uninstall):
//   1. InstallInitialize
//   2. InstallFiles
//   3. ImportTomlConfig      <-- Here, the exe and every dependency are on disk by now.
//   4. InstallFinalize
//
// All locals are declared at routine scope without initializers that a `goto`
// could skip (the WiX WcaUtil convention uses `goto LExit`).
UINT __stdcall ImportTomlConfig(
    __in MSIHANDLE hInstall)
{
    HRESULT hr = S_OK;
    DWORD er = ERROR_SUCCESS;

    LPWSTR pwzData = NULL;
    PWTS_SESSION_INFOW pSessions = NULL;
    LPWSTR pwzSep1 = NULL;
    LPWSTR pwzSep2 = NULL;
    LPCWSTR msiPath = NULL;
    LPCWSTR exePath = NULL;
    LPCWSTR tomlFileName = NULL;
    LPCWSTR pwzLastSep = NULL;
    WCHAR tomlPath[2048] = { 0 };
    DWORD sessionCount = 0;
    bool importedAny = false;

    hr = WcaInitialize(hInstall, "ImportTomlConfig");
    ExitOnFailure(hr, "Failed to initialize");

    hr = WcaGetProperty(L"CustomActionData", &pwzData);
    ExitOnFailure(hr, "failed to get CustomActionData");

    // Parse `<msi full path>|<installed exe full path>|<toml file name>`.
    // '|' never appears in Windows paths, so it is safe as a separator.
    pwzSep1 = wcschr(pwzData, L'|');
    pwzSep2 = (pwzSep1 == NULL) ? NULL : wcschr(pwzSep1 + 1, L'|');
    if (pwzSep1 == NULL || pwzSep2 == NULL) {
        WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: unexpected CustomActionData: %ls", pwzData);
        goto LExit;
    }
    *pwzSep1 = L'\0';
    *pwzSep2 = L'\0';
    msiPath = pwzData;
    exePath = pwzSep1 + 1;
    tomlFileName = pwzSep2 + 1;

    WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: msi: %ls", msiPath);
    WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: exe: %ls", exePath);
    WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: toml file name: %ls", tomlFileName);

    // The toml file is looked up next to the MSI.
    {
        LPCWSTR p = NULL;
        for (p = msiPath + wcslen(msiPath); p > msiPath; p--) {
            if (*(p - 1) == L'\\' || *(p - 1) == L'/') {
                pwzLastSep = p - 1;
                break;
            }
        }
        if (pwzLastSep == NULL) {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: cannot get the directory of the MSI, skip import.");
            goto LExit;
        }

        hr = StringCchPrintfW(tomlPath, ARRAYSIZE(tomlPath), L"%.*ls%ls", (int)(pwzLastSep - msiPath + 1), msiPath, tomlFileName);
        ExitOnFailure(hr, "ImportTomlConfig: failed to compose the toml path");

        if (!FileExistsW(tomlPath)) {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: '%ls' is not found next to the MSI, skip import.", tomlPath);
            goto LExit;
        }
        if (!FileExistsW(exePath)) {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: '%ls' is not found, skip import.", exePath);
            goto LExit;
        }

        // Run the import for every active (logged-in) user session. This covers
        // both the physical console and active RDP sessions. The user profile
        // is already loaded for an active session, so no LoadUserProfile call.
        if (!WTSEnumerateSessionsW(WTS_CURRENT_SERVER_HANDLE, 0, 1, &pSessions, &sessionCount)) {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: WTSEnumerateSessions failed, error: %lu, skip import.", GetLastError());
            goto LExit;
        }

        for (DWORD i = 0; i < sessionCount; ++i) {
            if (pSessions[i].State != WTSActive) {
                continue;
            }
            HANDLE hUserToken = NULL;
            if (!WTSQueryUserToken(pSessions[i].SessionId, &hUserToken)) {
                WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: WTSQueryUserToken failed for session %lu, error: %lu.", pSessions[i].SessionId, GetLastError());
                continue;
            }
            RunImportAsUser(hUserToken, pSessions[i].SessionId, exePath, tomlPath);
            CloseHandle(hUserToken);
            importedAny = true;
        }

        if (!importedAny) {
            WcaLog(LOGMSG_STANDARD, "ImportTomlConfig: no active user session found, skip import.");
        }
    }

LExit:
    if (pSessions != NULL) {
        WTSFreeMemory(pSessions);
    }
    if (pwzData) {
        ReleaseStr(pwzData);
    }

    er = SUCCEEDED(hr) ? ERROR_SUCCESS : ERROR_INSTALL_FAILURE;
    return WcaFinalize(er);
}
