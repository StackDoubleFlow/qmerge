using AsmResolver.DotNet;
using AsmResolver.PE.DotNet.Metadata.Tables.Rows;

namespace DllsGen;

public class SmallFixes
{
    /// <summary>
    /// For some unknown reason, type signatures are sometimes encoded as ValueType for System.IntPtr instead of just
    /// being encoded as IntPtr directly. This method will attempt to fix that.
    /// </summary>
    /// <param name="module">the module to look through and fix.</param>
    public static void FixIntPtrInModule(ModuleDefinition module)
    {
        foreach (var type in module.TopLevelTypes)
        {
            foreach (var methodDefinition in type.Methods)
            {
                var sig = methodDefinition.Signature;
                if (sig.ReturnType.ElementType == ElementType.ValueType && sig.ReturnType.Namespace == "System" &&
                    sig.ReturnType.Name == "IntPtr")
                {
                    Console.WriteLine("Fixing IntPtr in " + methodDefinition.Module.FilePath);
                    Console.WriteLine("For method " + methodDefinition);
                    sig.ReturnType = module.CorLibTypeFactory.IntPtr;
                }

                for (var i = 0; i < sig.ParameterTypes.Count; i++)
                {
                    var paramType = sig.ParameterTypes[i];
                    if (paramType.IsValueType && paramType.Namespace == "System" && paramType.Name == "IntPtr")
                    {
                        sig.ParameterTypes[i] = module.CorLibTypeFactory.IntPtr;
                    }

                    if (paramType.IsValueType && paramType.Namespace == "System" && paramType.Name == "UIntPtr")
                    {
                        sig.ParameterTypes[i] = module.CorLibTypeFactory.UIntPtr;
                    }
                }
            }
        }
    }
}