'use strict';

// Generated from src/kesnet_data.kes by tools/export_kesnet_metadata.js.
const entries = [
  {
    "name": "TensorDataset",
    "module": "std.kesnet.data",
    "gated": true,
    "type": true,
    "signature": "TensorDataset",
    "parameters": [],
    "documentation": "Imported TensorDataset from std.kesnet.data."
  },
  {
    "name": "features",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "TensorDataset",
    "field": true,
    "signature": "TensorDataset.features : DynTensor[double]",
    "parameters": [],
    "documentation": "Field features of TensorDataset."
  },
  {
    "name": "targets",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "TensorDataset",
    "field": true,
    "signature": "TensorDataset.targets : DynTensor[double]",
    "parameters": [],
    "documentation": "Field targets of TensorDataset."
  },
  {
    "name": "TensorDataset",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "signature": "TensorDataset(move features : DynTensor[double], move targets : DynTensor[double]) : TensorDataset",
    "parameters": [
      "move features : DynTensor[double]",
      "move targets : DynTensor[double]"
    ],
    "documentation": "Constructs a TensorDataset."
  },
  {
    "name": "length",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "TensorDataset",
    "signature": "TensorDataset.length() : Int",
    "parameters": [],
    "documentation": "Member of TensorDataset."
  },
  {
    "name": "featureCount",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "TensorDataset",
    "signature": "TensorDataset.featureCount() : Int",
    "parameters": [],
    "documentation": "Member of TensorDataset."
  },
  {
    "name": "targetCount",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "TensorDataset",
    "signature": "TensorDataset.targetCount() : Int",
    "parameters": [],
    "documentation": "Member of TensorDataset."
  },
  {
    "name": "targetRank",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "TensorDataset",
    "signature": "TensorDataset.targetRank() : Int",
    "parameters": [],
    "documentation": "Member of TensorDataset."
  },
  {
    "name": "DataLoader",
    "module": "std.kesnet.data",
    "gated": true,
    "type": true,
    "signature": "DataLoader",
    "parameters": [],
    "documentation": "Imported DataLoader from std.kesnet.data."
  },
  {
    "name": "dataset",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "DataLoader",
    "field": true,
    "signature": "DataLoader.dataset : TensorDataset",
    "parameters": [],
    "documentation": "Field dataset of DataLoader."
  },
  {
    "name": "batchSize",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "DataLoader",
    "field": true,
    "signature": "DataLoader.batchSize : Int",
    "parameters": [],
    "documentation": "Field batchSize of DataLoader."
  },
  {
    "name": "shuffle",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "DataLoader",
    "field": true,
    "signature": "DataLoader.shuffle : Bool",
    "parameters": [],
    "documentation": "Field shuffle of DataLoader."
  },
  {
    "name": "dropLast",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "DataLoader",
    "field": true,
    "signature": "DataLoader.dropLast : Bool",
    "parameters": [],
    "documentation": "Field dropLast of DataLoader."
  },
  {
    "name": "seed",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "DataLoader",
    "field": true,
    "signature": "DataLoader.seed : Int",
    "parameters": [],
    "documentation": "Field seed of DataLoader."
  },
  {
    "name": "epoch",
    "module": "std.kesnet.data",
    "gated": true,
    "receiver": "DataLoader",
    "field": true,
    "signature": "DataLoader.epoch : Int",
    "parameters": [],
    "documentation": "Field epoch of DataLoader."
  },
  {
    "name": "DataLoader",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "signature": "DataLoader(move dataset : TensorDataset, batchSize : Int = 32, shuffle : Bool = false, dropLast : Bool = false, seed : Int = 0) : DataLoader",
    "parameters": [
      "move dataset : TensorDataset",
      "batchSize : Int = 32",
      "shuffle : Bool = false",
      "dropLast : Bool = false",
      "seed : Int = 0"
    ],
    "documentation": "Constructs a DataLoader."
  },
  {
    "name": "batches",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "DataLoader",
    "signature": "DataLoader.batches() : Array[(Variable, Variable)]",
    "parameters": [],
    "documentation": "Member of DataLoader."
  },
  {
    "name": "length",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "DataLoader",
    "signature": "DataLoader.length() : Int",
    "parameters": [],
    "documentation": "Member of DataLoader."
  },
  {
    "name": "reset",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "receiver": "DataLoader",
    "signature": "DataLoader.reset() : void",
    "parameters": [],
    "documentation": "Member of DataLoader."
  },
  {
    "name": "fromDataFrame",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "signature": "fromDataFrame(frame : DataFrame, features : List[String], target : String) : TensorDataset or Error",
    "parameters": [
      "frame : DataFrame",
      "features : List[String]",
      "target : String"
    ],
    "documentation": "Free function from std.kesnet.data."
  },
  {
    "name": "trainTestSplit",
    "module": "std.kesnet.data",
    "gated": true,
    "named": true,
    "signature": "trainTestSplit(dataset : TensorDataset, testCount : Int, seed : Int = 0) : (TensorDataset, TensorDataset)",
    "parameters": [
      "dataset : TensorDataset",
      "testCount : Int",
      "seed : Int = 0"
    ],
    "documentation": "Free function from std.kesnet.data."
  }
];

const names = [
  "TensorDataset",
  "features",
  "targets",
  "length",
  "featureCount",
  "targetCount",
  "targetRank",
  "DataLoader",
  "dataset",
  "batchSize",
  "shuffle",
  "dropLast",
  "seed",
  "epoch",
  "batches",
  "reset",
  "fromDataFrame",
  "trainTestSplit"
];
const types = [
  "TensorDataset",
  "DataLoader"
];
const fieldTypes = new Map();
for (const entry of entries) {
  if (entry.field) fieldTypes.set(`${entry.receiver}.${entry.name}`, entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim());
}

const parents = {};

module.exports = { types, names, entries, fieldTypes, parents };
