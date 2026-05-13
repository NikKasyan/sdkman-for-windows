@echo off
if "%SDKMAN_WINDOWS_DIR%"=="" (
  set "SDKMAN_WINDOWS_DIR=%USERPROFILE%\.sdkman-windows"
)
set "SDK_EXE=%SDKMAN_WINDOWS_DIR%\bin\sdk.exe"
set "SDK_SHIMS=%SDKMAN_WINDOWS_DIR%\shims"
if not exist "%SDK_EXE%" (
  echo sdk.exe not found at "%SDK_EXE%"
  exit /b 1
)

if exist "%SDK_SHIMS%" (
  set "PATH=%SDK_SHIMS%;%PATH%"
)

if /I "%1"=="use" (
  for /f "delims=" %%L in ('"%SDK_EXE%" --emit-cmd %*') do %%L
  exit /b %ERRORLEVEL%
)

if /I "%1"=="env" if /I "%2"=="install" (
  for /f "delims=" %%L in ('"%SDK_EXE%" --emit-cmd %*') do %%L
  exit /b %ERRORLEVEL%
)

"%SDK_EXE%" %*
exit /b %ERRORLEVEL%
