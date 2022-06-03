using Mono.Cecil;

namespace DllsGen;

public static class VTableFixer
{
    private static void GetVirtualMethods(TypeDefinition typeDefinition, List<MethodReference> virtualMethods, ModuleDefinition module)
    {
        // if (typeDefinition.Module != module)
        // {
        //     return;
        // }
        
        foreach (var typeDef in typeDefinition.Interfaces.Select(interfaceImpl => interfaceImpl.InterfaceType.Resolve()))
        {
            GetVirtualMethods(typeDef, virtualMethods, module);
        }

        if (typeDefinition.BaseType != null)
        {
            GetVirtualMethods(typeDefinition.BaseType.Resolve(), virtualMethods, module);
        }

        virtualMethods.AddRange(typeDefinition.Methods.Where(method => method.IsVirtual && !method.IsStatic).Cast<MethodReference>());
    }

    private static void GetMethodImpls(TypeDefinition typeDefinition, List<MethodReference> methodImpls, ModuleDefinition module)
    {
        // if (typeDefinition.Module != module)
        // {
        //     return;
        // }
        if (typeDefinition.BaseType != null)
        {
            GetMethodImpls(typeDefinition.BaseType.Resolve(), methodImpls, module);
        }
        
        methodImpls.AddRange(typeDefinition.Methods.SelectMany(method => method.Overrides));
    }
    
    public static void FixType(TypeDefinition typeDefinition)
    {
        if (typeDefinition.BaseType == null || typeDefinition.IsArray || typeDefinition.IsInterface || typeDefinition.IsAbstract) return;
        var module = typeDefinition.Module;
        // if (typeDefinition.FullName != "System.IO.Stream/<CopyToAsyncInternal>d__27" || typeDefinition.FullName != "System.Threading.SemaphoreSlim/<WaitUntilCountOrTimeoutAsync>d__31") return;
        // if (typeDefinition.FullName.StartsWith("System.Collections.Concurrent"))
        // {
        //     Console.WriteLine(typeDefinition.FullName);
        // }
        // if (typeDefinition.FullName !=
        //     "System.Xml.XmlDownloadManager/<GetNonFileStreamAsync>d__5") return;
        
        var virtualMethods = new List<MethodReference>();
        GetVirtualMethods(typeDefinition, virtualMethods, module);
        
        // foreach (var virtualMethod in virtualMethods)
        // {
        //     Console.WriteLine(virtualMethod.FullName);
        // }
        
        // Console.WriteLine("test");

        var methodImpls = new List<MethodReference>();
        GetMethodImpls(typeDefinition, methodImpls, module);
        
        // foreach (var typeDefinitionMethod in methodImpls)
        // {
        //     Console.WriteLine(typeDefinitionMethod.FullName);
        // }
        
        foreach (var virtualMethod in virtualMethods.Except(methodImpls))
        {
            var declaringName = virtualMethod.DeclaringType.Resolve().Name;
            if (declaringName != "IAsyncStateMachine" && declaringName != "IEnumerator")
            {
                continue;
            }
            foreach (var method in typeDefinition.Methods)
            {
                // if (method.Name != "MoveNext")
                if (method.Name == virtualMethod.Name && method.Parameters.Count == virtualMethod.Parameters.Count && method.Parameters.Zip(virtualMethod.Parameters).All(p => p.Item1.ParameterType == p.Item2.ParameterType) && method.ReturnType == virtualMethod.ReturnType)
                {
                    if (method.Overrides.Count == 0)
                    {
                        // Console.WriteLine(method.Parameters.Count);
                        // Console.WriteLine("{0} {1}", method.FullName, method.Parameters.Count);
                        // Console.WriteLine("{0} {1}", virtualMethod.FullName, method.Parameters.Count);
                        method.Overrides.Add(virtualMethod);
                    }
                }
            }
        }
    }
}