#if DEBUG
using System;
using System.Diagnostics;
using System.IO;
using log4net;

namespace Obscura_Client;

internal static class DevServer
{
    private static readonly ILog Log = LogManager.GetLogger(typeof(DevServer));
    static Process? _process;
    public static readonly string PORT = "5021";

    public static void Start()
    {
        Log.Debug(Environment.CurrentDirectory);
        var obscuraUiDir = Path.GetFullPath(
            Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "..", "..", "..", "..", "obscura-ui"));
        var licensesJson = Path.GetFullPath(
            Path.Combine(AppContext.BaseDirectory, "..", "..", "..", "..", "..", "..", "..", "webui-build", "licenses.json"));

        if (!Directory.Exists(obscuraUiDir))
        {
            Log.Error($"Failed to start; obscura-ui directory not found at: {obscuraUiDir}");
            return;
        }

        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "cmd.exe",
                Arguments = $"/c pnpm start --port {PORT}",
                WorkingDirectory = obscuraUiDir,
                CreateNoWindow = true,
                RedirectStandardOutput = true,
                RedirectStandardError = true,
            };
            psi.Environment["OBS_WEB_PLATFORM"] = "windows";
            psi.Environment["LICENSE_JSON"] = licensesJson;
            _process = Process.Start(psi);

            if (_process != null)
            {
                _process.OutputDataReceived += (s, e) => { if (e.Data != null) Log.Debug(e.Data); };
                _process.ErrorDataReceived += (s, e) => { if (e.Data != null) Log.Error(e.Data); };
                _process.BeginOutputReadLine();
                _process.BeginErrorReadLine();
                Log.Info($"Started dev server (PID {_process.Id})");

                if (_process.HasExited)
                {
                    throw new InvalidOperationException($"Dev server exited immediately with code {_process.ExitCode}");
                }
            }

        }
        catch (Exception ex)
        {
            Log.Error($"Failed to start: {ex.Message}");
            throw;
        }
    }

    public static void Stop()
    {
        try
        {
            var psi = new ProcessStartInfo
            {
                FileName = "cmd.exe",
                Arguments = $"/c npx -y kill-port {PORT}",
                CreateNoWindow = true,
                UseShellExecute = false,
            };
            var killPortProcess = Process.Start(psi);
            killPortProcess?.WaitForExit();
            Log.Info("Stopped");
        }
        catch (Exception ex)
        {
            Log.Error($"Error stopping: {ex.Message}");
        }
        finally
        {
            _process?.Dispose();
            _process = null;
        }
    }
}

#endif
