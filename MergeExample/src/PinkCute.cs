using System;
using TMPro;
using UnityEngine;

namespace MergeExample
{
    public class PinkCute : MonoBehaviour
    {
        void Start()
        {
            InvokeRepeating(nameof(PerformPinkCute), 2.0f, 3.0f);
        }

        void PerformPinkCute()
        {
            Debug.Log("Performing the pink cute");
            var objects = Resources.FindObjectsOfTypeAll<TMP_Text>();
            foreach (var textMeshPro in objects)
            {
                textMeshPro.text = "pink cute";
            }
        }
    }
}