using AsmResolver.DotNet;
using AsmResolver.DotNet.Code.Cil;
using AsmResolver.DotNet.Collections;
using AsmResolver.PE.DotNet.Cil;

namespace DllsGen;

public class MethodCopier
{
    public static void CopyMethodBody(MethodDefinition fromMethod, MethodDefinition toMethod,
        ReferenceConverter converter)
    {
        var module = toMethod.Module;
        var body = new CilMethodBody(toMethod);
        var referenceBody = fromMethod.CilMethodBody;

        body.MaxStack = referenceBody.MaxStack;
        body.ComputeMaxStackOnBuild = false;
        body.InitializeLocals = referenceBody.InitializeLocals;

        foreach (var localVariable in referenceBody.LocalVariables)
        {
            body.LocalVariables.Add(new CilLocalVariable(converter.Convert(localVariable.VariableType)
                .ImportWith(module.DefaultImporter)));
        }

        foreach (var exceptionHandler in referenceBody.ExceptionHandlers)
        {
            ICilLabel? filterStart;
            if (exceptionHandler.FilterStart is { } l)
            {
                filterStart = l;
            }
            else
            {
                filterStart = null;
            }

            body.ExceptionHandlers.Add(new CilExceptionHandler
            {
                HandlerType = exceptionHandler.HandlerType,
                TryStart = new CilOffsetLabel(exceptionHandler.TryStart.Offset),
                TryEnd = new CilOffsetLabel(exceptionHandler.TryEnd.Offset),
                HandlerStart = new CilOffsetLabel(exceptionHandler.HandlerStart.Offset),
                HandlerEnd = new CilOffsetLabel(exceptionHandler.HandlerEnd.Offset),
                FilterStart = filterStart,
                ExceptionType = exceptionHandler.ExceptionType != null
                    ? converter.Convert(exceptionHandler.ExceptionType).ImportWith(module.DefaultImporter)
                    : null,
            });
        }

        foreach (var instruction in referenceBody.Instructions)
        {
            var newOperand = instruction.Operand;
            switch (instruction.Operand)
            {
                case MemberReference reference:
                {
                    if (reference.IsField)
                    {
                        newOperand = module.DefaultImporter.ImportField(converter.Convert(reference));
                    }
                    else if (reference.IsMethod)
                    {
                        newOperand = module.DefaultImporter.ImportMethod(converter.Convert(reference));
                    }

                    break;
                }
                case IMethodDescriptor descriptor:
                {
                    newOperand = module.DefaultImporter.ImportMethod(converter.Convert(descriptor));
                    break;
                }
                case IFieldDescriptor descriptor:
                {
                    newOperand = module.DefaultImporter.ImportField(converter.Convert(descriptor));
                    break;
                }
                case ITypeDefOrRef defOrRef:
                {
                    newOperand = module.DefaultImporter.ImportType(converter.Convert(defOrRef));
                    break;
                }
                case CilInstructionLabel label:
                    newOperand = new CilOffsetLabel(label.Offset);
                    break;
                case List<ICilLabel> list:
                    newOperand = list.Select(label => (ICilLabel) new CilOffsetLabel(label.Offset)).ToList();
                    break;
                case null:
                case sbyte:
                case int:
                case long:
                case float:
                case double:
                case string:
                case Parameter:
                case CilLocalVariable:
                    break;
                default:
                    Console.WriteLine($"Unhandled operand type while copying method {toMethod}: " +
                                      newOperand.GetType().FullName);
                    break;
            }

            body.Instructions.Add(new CilInstruction(instruction.OpCode, newOperand));
        }

        toMethod.CilMethodBody = body;
    }
}