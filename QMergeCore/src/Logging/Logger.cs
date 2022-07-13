using QMerge.Natives;

namespace QMerge.Logging
{
    public class Logger
    {
        private readonly string _tag;

        public Logger(string modId)
        {
            _tag = $"QMerge[{modId}]";
        }

        public enum LogPriority
        {
            Unknown = 0,
            Default = 1,
            Verbose = 2,
            Debug = 3,
            Info = 4,
            Warn = 5,
            Error = 6,
            Fatal = 7,
            Silent = 8
        }

        public void Log(LogPriority priority, string message)
        {
            LogMessageNative((int) priority, _tag, message);
        }

        public void Verbose(string message) => Log(LogPriority.Verbose, message);
        public void Debug(string message) => Log(LogPriority.Debug, message);
        public void Info(string message) => Log(LogPriority.Info, message);
        public void Warn(string message) => Log(LogPriority.Warn, message);
        public void Error(string message) => Log(LogPriority.Error, message);
        public void Fatal(string message) => Log(LogPriority.Fatal, message);
        public void Silent(string message) => Log(LogPriority.Silent, message);
        
        private static void LogMessageNative(int priority, string tag, string message)
        {
            NativeHelper.NativeStub(priority, tag, message);
        }
    }
}