using QMerge.Hooking;
using System.Reflection;
using UnityEngine;

namespace InvertedArrows.Hooks
{
    [Hook(typeof(GameNoteController), "Init",
        new[]
        {
            typeof(NoteData), typeof(float), typeof(Vector3), typeof(Vector3), typeof(Vector3), typeof(float),
            typeof(float), typeof(float), typeof(NoteVisualModifierType), typeof(float), typeof(float)
        })]
    class GameNoteControllerInit
    {
        public static byte[] directionLookup = {1, 0, 3, 2, 7, 6, 5, 4};

        public static void Prefix(NoteData noteData)
        {
            if ((int) noteData.cutDirection <= 7)
            {
                Plugin.Logger.Info("Changing cut direction to " + directionLookup[(int) noteData.cutDirection]);
                PropertyInfo? prop =
                    typeof(NoteData).GetProperty("cutDirection", BindingFlags.NonPublic | BindingFlags.Instance);
                prop.SetValue(noteData, directionLookup[(int) noteData.cutDirection], null);
            }
            else
            {
                Plugin.Logger.Info("Not changing cut direction!");
            }
        }
    }
}