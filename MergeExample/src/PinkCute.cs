using System;
using TMPro;
using UnityEngine;

namespace MergeExample
{
    public class PinkCute : MonoBehaviour
    {
        void Start()
        {
            InvokeRepeating(nameof(DoPinkCute), 2.0f, 3.0f);
        }

        void DoPinkCute()
        {
            Debug.Log("Doing the pink cute");
            var objects = Resources.FindObjectsOfTypeAll<TextMeshPro>();
            foreach (var textMeshPro in objects)
            {
                textMeshPro.text = "pink cute";
            }
        }
    }
}