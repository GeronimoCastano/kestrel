'use strict';

// Generated from src/kesnet.kes by tools/export_kesnet_metadata.js.
const entries = [
  {
    "name": "inputGates",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.inputGates : Linear",
    "parameters": [],
    "documentation": "Field inputGates of LSTMCell."
  },
  {
    "name": "hiddenGates",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hiddenGates : Linear",
    "parameters": [],
    "documentation": "Field hiddenGates of LSTMCell."
  },
  {
    "name": "inputSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTM",
    "field": true,
    "signature": "LSTM.inputSize : Int",
    "parameters": [],
    "documentation": "Field inputSize of LSTM."
  },
  {
    "name": "stepProjected",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "signature": "LSTMCell.stepProjected(projected : Variable, hidden : Variable, cell : Variable) : (Variable, Variable)",
    "parameters": [
      "projected : Variable",
      "hidden : Variable",
      "cell : Variable"
    ],
    "documentation": "Advances the cell from already projected input."
  },
  {
    "name": "Module",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Module",
    "parameters": [],
    "documentation": "Imported Module from std.kesnet."
  },
  {
    "name": "registered",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Module",
    "field": true,
    "signature": "Module.registered : Array[Variable]",
    "parameters": [],
    "documentation": "Field registered of Module."
  },
  {
    "name": "trainingMode",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Module",
    "field": true,
    "signature": "Module.trainingMode : Bool",
    "parameters": [],
    "documentation": "Field trainingMode of Module."
  },
  {
    "name": "Module",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Module() : Module",
    "parameters": [],
    "documentation": "Constructs a Module."
  },
  {
    "name": "addParameter",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.addParameter(parameter : Variable) : void",
    "parameters": [
      "parameter : Variable"
    ],
    "documentation": "Member of Module."
  },
  {
    "name": "parameters",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.parameters() : List[Variable]",
    "parameters": [],
    "documentation": "Member of Module."
  },
  {
    "name": "parameterCount",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.parameterCount() : Int",
    "parameters": [],
    "documentation": "Member of Module."
  },
  {
    "name": "train",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.train() : void",
    "parameters": [],
    "documentation": "Member of Module."
  },
  {
    "name": "eval",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.eval() : void",
    "parameters": [],
    "documentation": "Member of Module."
  },
  {
    "name": "zeroGrad",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.zeroGrad() : void",
    "parameters": [],
    "documentation": "Member of Module."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Module."
  },
  {
    "name": "summary",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Module",
    "signature": "Module.summary() : String",
    "parameters": [],
    "documentation": "Member of Module."
  },
  {
    "name": "Identity",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Identity",
    "parameters": [],
    "documentation": "Imported Identity from std.kesnet."
  },
  {
    "name": "Identity",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Identity() : Identity",
    "parameters": [],
    "documentation": "Constructs a Identity."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Identity",
    "signature": "Identity.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Identity."
  },
  {
    "name": "Linear",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Linear",
    "parameters": [],
    "documentation": "Imported Linear from std.kesnet."
  },
  {
    "name": "weight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Linear",
    "field": true,
    "signature": "Linear.weight : Variable",
    "parameters": [],
    "documentation": "Field weight of Linear."
  },
  {
    "name": "biasValue",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Linear",
    "field": true,
    "signature": "Linear.biasValue : Variable",
    "parameters": [],
    "documentation": "Field biasValue of Linear."
  },
  {
    "name": "hasBias",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Linear",
    "field": true,
    "signature": "Linear.hasBias : Bool",
    "parameters": [],
    "documentation": "Field hasBias of Linear."
  },
  {
    "name": "inFeatures",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Linear",
    "field": true,
    "signature": "Linear.inFeatures : Int",
    "parameters": [],
    "documentation": "Field inFeatures of Linear."
  },
  {
    "name": "outFeatures",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Linear",
    "field": true,
    "signature": "Linear.outFeatures : Int",
    "parameters": [],
    "documentation": "Field outFeatures of Linear."
  },
  {
    "name": "Linear",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Linear(inFeatures : Int, outFeatures : Int, bias : Bool = true) : Linear",
    "parameters": [
      "inFeatures : Int",
      "outFeatures : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a Linear."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Linear",
    "signature": "Linear.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Linear."
  },
  {
    "name": "ReLU",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "ReLU",
    "parameters": [],
    "documentation": "Imported ReLU from std.kesnet."
  },
  {
    "name": "ReLU",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "ReLU() : ReLU",
    "parameters": [],
    "documentation": "Constructs a ReLU."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "ReLU",
    "signature": "ReLU.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of ReLU."
  },
  {
    "name": "LeakyReLU",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "LeakyReLU",
    "parameters": [],
    "documentation": "Imported LeakyReLU from std.kesnet."
  },
  {
    "name": "slope",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LeakyReLU",
    "field": true,
    "signature": "LeakyReLU.slope : double",
    "parameters": [],
    "documentation": "Field slope of LeakyReLU."
  },
  {
    "name": "LeakyReLU",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "LeakyReLU(slope : double = 0.01) : LeakyReLU",
    "parameters": [
      "slope : double = 0.01"
    ],
    "documentation": "Constructs a LeakyReLU."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "LeakyReLU",
    "signature": "LeakyReLU.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of LeakyReLU."
  },
  {
    "name": "ELU",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "ELU",
    "parameters": [],
    "documentation": "Imported ELU from std.kesnet."
  },
  {
    "name": "alpha",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "ELU",
    "field": true,
    "signature": "ELU.alpha : double",
    "parameters": [],
    "documentation": "Field alpha of ELU."
  },
  {
    "name": "ELU",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "ELU(alpha : double = 1.0) : ELU",
    "parameters": [
      "alpha : double = 1.0"
    ],
    "documentation": "Constructs a ELU."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "ELU",
    "signature": "ELU.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of ELU."
  },
  {
    "name": "GELU",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "GELU",
    "parameters": [],
    "documentation": "Imported GELU from std.kesnet."
  },
  {
    "name": "GELU",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "GELU() : GELU",
    "parameters": [],
    "documentation": "Constructs a GELU."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "GELU",
    "signature": "GELU.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of GELU."
  },
  {
    "name": "SiLU",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "SiLU",
    "parameters": [],
    "documentation": "Imported SiLU from std.kesnet."
  },
  {
    "name": "SiLU",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "SiLU() : SiLU",
    "parameters": [],
    "documentation": "Constructs a SiLU."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "SiLU",
    "signature": "SiLU.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of SiLU."
  },
  {
    "name": "Sigmoid",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Sigmoid",
    "parameters": [],
    "documentation": "Imported Sigmoid from std.kesnet."
  },
  {
    "name": "Sigmoid",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Sigmoid() : Sigmoid",
    "parameters": [],
    "documentation": "Constructs a Sigmoid."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Sigmoid",
    "signature": "Sigmoid.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Sigmoid."
  },
  {
    "name": "Tanh",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Tanh",
    "parameters": [],
    "documentation": "Imported Tanh from std.kesnet."
  },
  {
    "name": "Tanh",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Tanh() : Tanh",
    "parameters": [],
    "documentation": "Constructs a Tanh."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Tanh",
    "signature": "Tanh.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Tanh."
  },
  {
    "name": "Softmax",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Softmax",
    "parameters": [],
    "documentation": "Imported Softmax from std.kesnet."
  },
  {
    "name": "axis",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Softmax",
    "field": true,
    "signature": "Softmax.axis : Int",
    "parameters": [],
    "documentation": "Field axis of Softmax."
  },
  {
    "name": "Softmax",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Softmax(axis : Int = 0 - 1) : Softmax",
    "parameters": [
      "axis : Int = 0 - 1"
    ],
    "documentation": "Constructs a Softmax."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Softmax",
    "signature": "Softmax.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Softmax."
  },
  {
    "name": "LogSoftmax",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "LogSoftmax",
    "parameters": [],
    "documentation": "Imported LogSoftmax from std.kesnet."
  },
  {
    "name": "axis",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LogSoftmax",
    "field": true,
    "signature": "LogSoftmax.axis : Int",
    "parameters": [],
    "documentation": "Field axis of LogSoftmax."
  },
  {
    "name": "LogSoftmax",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "LogSoftmax(axis : Int = 0 - 1) : LogSoftmax",
    "parameters": [
      "axis : Int = 0 - 1"
    ],
    "documentation": "Constructs a LogSoftmax."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "LogSoftmax",
    "signature": "LogSoftmax.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of LogSoftmax."
  },
  {
    "name": "Flatten",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Flatten",
    "parameters": [],
    "documentation": "Imported Flatten from std.kesnet."
  },
  {
    "name": "startDim",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Flatten",
    "field": true,
    "signature": "Flatten.startDim : Int",
    "parameters": [],
    "documentation": "Field startDim of Flatten."
  },
  {
    "name": "Flatten",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Flatten(startDim : Int = 1) : Flatten",
    "parameters": [
      "startDim : Int = 1"
    ],
    "documentation": "Constructs a Flatten."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Flatten",
    "signature": "Flatten.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Flatten."
  },
  {
    "name": "Dropout",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Dropout",
    "parameters": [],
    "documentation": "Imported Dropout from std.kesnet."
  },
  {
    "name": "probability",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Dropout",
    "field": true,
    "signature": "Dropout.probability : double",
    "parameters": [],
    "documentation": "Field probability of Dropout."
  },
  {
    "name": "Dropout",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Dropout(probability : double = 0.5) : Dropout",
    "parameters": [
      "probability : double = 0.5"
    ],
    "documentation": "Constructs a Dropout."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Dropout",
    "signature": "Dropout.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Dropout."
  },
  {
    "name": "Sequential",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Sequential",
    "parameters": [],
    "documentation": "Imported Sequential from std.kesnet."
  },
  {
    "name": "layers",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Sequential",
    "field": true,
    "signature": "Sequential.layers : List[Box[dyn Network]]",
    "parameters": [],
    "documentation": "Field layers of Sequential."
  },
  {
    "name": "Sequential",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Sequential(layers : List[Box[dyn Network]]) : Sequential",
    "parameters": [
      "layers : List[Box[dyn Network]]"
    ],
    "documentation": "Constructs a Sequential."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Sequential",
    "signature": "Sequential.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Sequential."
  },
  {
    "name": "ModuleList",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "ModuleList",
    "parameters": [],
    "documentation": "Imported ModuleList from std.kesnet."
  },
  {
    "name": "layers",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "ModuleList",
    "field": true,
    "signature": "ModuleList.layers : List[Box[dyn Network]]",
    "parameters": [],
    "documentation": "Field layers of ModuleList."
  },
  {
    "name": "ModuleList",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "ModuleList(layers : List[Box[dyn Network]]) : ModuleList",
    "parameters": [
      "layers : List[Box[dyn Network]]"
    ],
    "documentation": "Constructs a ModuleList."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "ModuleList",
    "signature": "ModuleList.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of ModuleList."
  },
  {
    "name": "MSELoss",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "MSELoss",
    "parameters": [],
    "documentation": "Imported MSELoss from std.kesnet."
  },
  {
    "name": "reduction",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MSELoss",
    "field": true,
    "signature": "MSELoss.reduction : String",
    "parameters": [],
    "documentation": "Field reduction of MSELoss."
  },
  {
    "name": "MSELoss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "MSELoss(reduction : String = \"mean\") : MSELoss",
    "parameters": [
      "reduction : String = \"mean\""
    ],
    "documentation": "Constructs a MSELoss."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "MSELoss",
    "signature": "MSELoss.forward(prediction : Variable, target : Variable) : Variable",
    "parameters": [
      "prediction : Variable",
      "target : Variable"
    ],
    "documentation": "Member of MSELoss."
  },
  {
    "name": "L1Loss",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "L1Loss",
    "parameters": [],
    "documentation": "Imported L1Loss from std.kesnet."
  },
  {
    "name": "reduction",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "L1Loss",
    "field": true,
    "signature": "L1Loss.reduction : String",
    "parameters": [],
    "documentation": "Field reduction of L1Loss."
  },
  {
    "name": "L1Loss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "L1Loss(reduction : String = \"mean\") : L1Loss",
    "parameters": [
      "reduction : String = \"mean\""
    ],
    "documentation": "Constructs a L1Loss."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "L1Loss",
    "signature": "L1Loss.forward(prediction : Variable, target : Variable) : Variable",
    "parameters": [
      "prediction : Variable",
      "target : Variable"
    ],
    "documentation": "Member of L1Loss."
  },
  {
    "name": "CrossEntropyLoss",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "CrossEntropyLoss",
    "parameters": [],
    "documentation": "Imported CrossEntropyLoss from std.kesnet."
  },
  {
    "name": "CrossEntropyLoss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "CrossEntropyLoss() : CrossEntropyLoss",
    "parameters": [],
    "documentation": "Constructs a CrossEntropyLoss."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "CrossEntropyLoss",
    "signature": "CrossEntropyLoss.forward(logits : Variable, target : Variable) : Variable",
    "parameters": [
      "logits : Variable",
      "target : Variable"
    ],
    "documentation": "Member of CrossEntropyLoss."
  },
  {
    "name": "NLLLoss",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "NLLLoss",
    "parameters": [],
    "documentation": "Imported NLLLoss from std.kesnet."
  },
  {
    "name": "NLLLoss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "NLLLoss() : NLLLoss",
    "parameters": [],
    "documentation": "Constructs a NLLLoss."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "NLLLoss",
    "signature": "NLLLoss.forward(logProbabilities : Variable, target : Variable) : Variable",
    "parameters": [
      "logProbabilities : Variable",
      "target : Variable"
    ],
    "documentation": "Member of NLLLoss."
  },
  {
    "name": "BCEWithLogitsLoss",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "BCEWithLogitsLoss",
    "parameters": [],
    "documentation": "Imported BCEWithLogitsLoss from std.kesnet."
  },
  {
    "name": "BCEWithLogitsLoss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "BCEWithLogitsLoss() : BCEWithLogitsLoss",
    "parameters": [],
    "documentation": "Constructs a BCEWithLogitsLoss."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "BCEWithLogitsLoss",
    "signature": "BCEWithLogitsLoss.forward(logits : Variable, target : Variable) : Variable",
    "parameters": [
      "logits : Variable",
      "target : Variable"
    ],
    "documentation": "Member of BCEWithLogitsLoss."
  },
  {
    "name": "relu",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "relu(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "sigmoid",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "sigmoid(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "tanh",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "tanh(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "gelu",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "gelu(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "silu",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "silu(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "softmax",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "softmax(x : Variable, axis : Int = 0 - 1) : Variable",
    "parameters": [
      "x : Variable",
      "axis : Int = 0 - 1"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "logSoftmax",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "logSoftmax(x : Variable, axis : Int = 0 - 1) : Variable",
    "parameters": [
      "x : Variable",
      "axis : Int = 0 - 1"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "linear",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "linear(x : Variable, weight : Variable, bias : Option[Variable] = None) : Variable",
    "parameters": [
      "x : Variable",
      "weight : Variable",
      "bias : Option[Variable] = None"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "mseLoss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "mseLoss(prediction : Variable, target : Variable) : Variable",
    "parameters": [
      "prediction : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "l1Loss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "l1Loss(prediction : Variable, target : Variable) : Variable",
    "parameters": [
      "prediction : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "crossEntropy",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "crossEntropy(logits : Variable, target : Variable) : Variable",
    "parameters": [
      "logits : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "nllLoss",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "nllLoss(logProbabilities : Variable, target : Variable) : Variable",
    "parameters": [
      "logProbabilities : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "binaryCrossEntropy",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "binaryCrossEntropy(logits : Variable, target : Variable) : Variable",
    "parameters": [
      "logits : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "Conv2d",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Conv2d",
    "parameters": [],
    "documentation": "Imported Conv2d from std.kesnet."
  },
  {
    "name": "weight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.weight : Variable",
    "parameters": [],
    "documentation": "Field weight of Conv2d."
  },
  {
    "name": "biasValue",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.biasValue : Variable",
    "parameters": [],
    "documentation": "Field biasValue of Conv2d."
  },
  {
    "name": "hasBias",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.hasBias : Bool",
    "parameters": [],
    "documentation": "Field hasBias of Conv2d."
  },
  {
    "name": "inChannels",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.inChannels : Int",
    "parameters": [],
    "documentation": "Field inChannels of Conv2d."
  },
  {
    "name": "outChannels",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.outChannels : Int",
    "parameters": [],
    "documentation": "Field outChannels of Conv2d."
  },
  {
    "name": "kernelSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.kernelSize : Int",
    "parameters": [],
    "documentation": "Field kernelSize of Conv2d."
  },
  {
    "name": "stride",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.stride : Int",
    "parameters": [],
    "documentation": "Field stride of Conv2d."
  },
  {
    "name": "padding",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.padding : Int",
    "parameters": [],
    "documentation": "Field padding of Conv2d."
  },
  {
    "name": "dilation",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.dilation : Int",
    "parameters": [],
    "documentation": "Field dilation of Conv2d."
  },
  {
    "name": "groups",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Conv2d",
    "field": true,
    "signature": "Conv2d.groups : Int",
    "parameters": [],
    "documentation": "Field groups of Conv2d."
  },
  {
    "name": "Conv2d",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Conv2d(inChannels : Int, outChannels : Int, kernelSize : Int, stride : Int = 1, padding : Int = 0, dilation : Int = 1, groups : Int = 1, bias : Bool = true) : Conv2d",
    "parameters": [
      "inChannels : Int",
      "outChannels : Int",
      "kernelSize : Int",
      "stride : Int = 1",
      "padding : Int = 0",
      "dilation : Int = 1",
      "groups : Int = 1",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a Conv2d."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Conv2d",
    "signature": "Conv2d.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Conv2d."
  },
  {
    "name": "MaxPool2d",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "MaxPool2d",
    "parameters": [],
    "documentation": "Imported MaxPool2d from std.kesnet."
  },
  {
    "name": "kernelSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MaxPool2d",
    "field": true,
    "signature": "MaxPool2d.kernelSize : Int",
    "parameters": [],
    "documentation": "Field kernelSize of MaxPool2d."
  },
  {
    "name": "stride",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MaxPool2d",
    "field": true,
    "signature": "MaxPool2d.stride : Int",
    "parameters": [],
    "documentation": "Field stride of MaxPool2d."
  },
  {
    "name": "padding",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MaxPool2d",
    "field": true,
    "signature": "MaxPool2d.padding : Int",
    "parameters": [],
    "documentation": "Field padding of MaxPool2d."
  },
  {
    "name": "MaxPool2d",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "MaxPool2d(kernelSize : Int, stride : Int = 0, padding : Int = 0) : MaxPool2d",
    "parameters": [
      "kernelSize : Int",
      "stride : Int = 0",
      "padding : Int = 0"
    ],
    "documentation": "Constructs a MaxPool2d."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "MaxPool2d",
    "signature": "MaxPool2d.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of MaxPool2d."
  },
  {
    "name": "AvgPool2d",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "AvgPool2d",
    "parameters": [],
    "documentation": "Imported AvgPool2d from std.kesnet."
  },
  {
    "name": "kernelSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "AvgPool2d",
    "field": true,
    "signature": "AvgPool2d.kernelSize : Int",
    "parameters": [],
    "documentation": "Field kernelSize of AvgPool2d."
  },
  {
    "name": "stride",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "AvgPool2d",
    "field": true,
    "signature": "AvgPool2d.stride : Int",
    "parameters": [],
    "documentation": "Field stride of AvgPool2d."
  },
  {
    "name": "padding",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "AvgPool2d",
    "field": true,
    "signature": "AvgPool2d.padding : Int",
    "parameters": [],
    "documentation": "Field padding of AvgPool2d."
  },
  {
    "name": "AvgPool2d",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "AvgPool2d(kernelSize : Int, stride : Int = 0, padding : Int = 0) : AvgPool2d",
    "parameters": [
      "kernelSize : Int",
      "stride : Int = 0",
      "padding : Int = 0"
    ],
    "documentation": "Constructs a AvgPool2d."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "AvgPool2d",
    "signature": "AvgPool2d.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of AvgPool2d."
  },
  {
    "name": "AdaptiveAvgPool2d",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "AdaptiveAvgPool2d",
    "parameters": [],
    "documentation": "Imported AdaptiveAvgPool2d from std.kesnet."
  },
  {
    "name": "outHeight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "AdaptiveAvgPool2d",
    "field": true,
    "signature": "AdaptiveAvgPool2d.outHeight : Int",
    "parameters": [],
    "documentation": "Field outHeight of AdaptiveAvgPool2d."
  },
  {
    "name": "outWidth",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "AdaptiveAvgPool2d",
    "field": true,
    "signature": "AdaptiveAvgPool2d.outWidth : Int",
    "parameters": [],
    "documentation": "Field outWidth of AdaptiveAvgPool2d."
  },
  {
    "name": "AdaptiveAvgPool2d",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "AdaptiveAvgPool2d(outHeight : Int, outWidth : Int) : AdaptiveAvgPool2d",
    "parameters": [
      "outHeight : Int",
      "outWidth : Int"
    ],
    "documentation": "Constructs a AdaptiveAvgPool2d."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "AdaptiveAvgPool2d",
    "signature": "AdaptiveAvgPool2d.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of AdaptiveAvgPool2d."
  },
  {
    "name": "BatchNorm2d",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "BatchNorm2d",
    "parameters": [],
    "documentation": "Imported BatchNorm2d from std.kesnet."
  },
  {
    "name": "weight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.weight : Variable",
    "parameters": [],
    "documentation": "Field weight of BatchNorm2d."
  },
  {
    "name": "biasValue",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.biasValue : Variable",
    "parameters": [],
    "documentation": "Field biasValue of BatchNorm2d."
  },
  {
    "name": "runningMean",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.runningMean : Variable",
    "parameters": [],
    "documentation": "Field runningMean of BatchNorm2d."
  },
  {
    "name": "runningVar",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.runningVar : Variable",
    "parameters": [],
    "documentation": "Field runningVar of BatchNorm2d."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of BatchNorm2d."
  },
  {
    "name": "momentum",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.momentum : double",
    "parameters": [],
    "documentation": "Field momentum of BatchNorm2d."
  },
  {
    "name": "affine",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm2d",
    "field": true,
    "signature": "BatchNorm2d.affine : Bool",
    "parameters": [],
    "documentation": "Field affine of BatchNorm2d."
  },
  {
    "name": "BatchNorm2d",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "BatchNorm2d(numFeatures : Int, epsilon : double = 0.00001, momentum : double = 0.1, affine : Bool = true) : BatchNorm2d",
    "parameters": [
      "numFeatures : Int",
      "epsilon : double = 0.00001",
      "momentum : double = 0.1",
      "affine : Bool = true"
    ],
    "documentation": "Constructs a BatchNorm2d."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "BatchNorm2d",
    "signature": "BatchNorm2d.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of BatchNorm2d."
  },
  {
    "name": "BatchNorm1d",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "BatchNorm1d",
    "parameters": [],
    "documentation": "Imported BatchNorm1d from std.kesnet."
  },
  {
    "name": "inner",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "BatchNorm1d",
    "field": true,
    "signature": "BatchNorm1d.inner : BatchNorm2d",
    "parameters": [],
    "documentation": "Field inner of BatchNorm1d."
  },
  {
    "name": "BatchNorm1d",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "BatchNorm1d(numFeatures : Int, epsilon : double = 0.00001, momentum : double = 0.1, affine : Bool = true) : BatchNorm1d",
    "parameters": [
      "numFeatures : Int",
      "epsilon : double = 0.00001",
      "momentum : double = 0.1",
      "affine : Bool = true"
    ],
    "documentation": "Constructs a BatchNorm1d."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "BatchNorm1d",
    "signature": "BatchNorm1d.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of BatchNorm1d."
  },
  {
    "name": "LayerNorm",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "LayerNorm",
    "parameters": [],
    "documentation": "Imported LayerNorm from std.kesnet."
  },
  {
    "name": "weight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LayerNorm",
    "field": true,
    "signature": "LayerNorm.weight : Variable",
    "parameters": [],
    "documentation": "Field weight of LayerNorm."
  },
  {
    "name": "biasValue",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LayerNorm",
    "field": true,
    "signature": "LayerNorm.biasValue : Variable",
    "parameters": [],
    "documentation": "Field biasValue of LayerNorm."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LayerNorm",
    "field": true,
    "signature": "LayerNorm.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of LayerNorm."
  },
  {
    "name": "affine",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LayerNorm",
    "field": true,
    "signature": "LayerNorm.affine : Bool",
    "parameters": [],
    "documentation": "Field affine of LayerNorm."
  },
  {
    "name": "LayerNorm",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "LayerNorm(normalizedSize : Int, epsilon : double = 0.00001, affine : Bool = true) : LayerNorm",
    "parameters": [
      "normalizedSize : Int",
      "epsilon : double = 0.00001",
      "affine : Bool = true"
    ],
    "documentation": "Constructs a LayerNorm."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "LayerNorm",
    "signature": "LayerNorm.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of LayerNorm."
  },
  {
    "name": "GroupNorm",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "GroupNorm",
    "parameters": [],
    "documentation": "Imported GroupNorm from std.kesnet."
  },
  {
    "name": "weight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GroupNorm",
    "field": true,
    "signature": "GroupNorm.weight : Variable",
    "parameters": [],
    "documentation": "Field weight of GroupNorm."
  },
  {
    "name": "biasValue",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GroupNorm",
    "field": true,
    "signature": "GroupNorm.biasValue : Variable",
    "parameters": [],
    "documentation": "Field biasValue of GroupNorm."
  },
  {
    "name": "numGroups",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GroupNorm",
    "field": true,
    "signature": "GroupNorm.numGroups : Int",
    "parameters": [],
    "documentation": "Field numGroups of GroupNorm."
  },
  {
    "name": "numChannels",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GroupNorm",
    "field": true,
    "signature": "GroupNorm.numChannels : Int",
    "parameters": [],
    "documentation": "Field numChannels of GroupNorm."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GroupNorm",
    "field": true,
    "signature": "GroupNorm.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of GroupNorm."
  },
  {
    "name": "affine",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GroupNorm",
    "field": true,
    "signature": "GroupNorm.affine : Bool",
    "parameters": [],
    "documentation": "Field affine of GroupNorm."
  },
  {
    "name": "GroupNorm",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "GroupNorm(numGroups : Int, numChannels : Int, epsilon : double = 0.00001, affine : Bool = true) : GroupNorm",
    "parameters": [
      "numGroups : Int",
      "numChannels : Int",
      "epsilon : double = 0.00001",
      "affine : Bool = true"
    ],
    "documentation": "Constructs a GroupNorm."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "GroupNorm",
    "signature": "GroupNorm.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of GroupNorm."
  },
  {
    "name": "Embedding",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "Embedding",
    "parameters": [],
    "documentation": "Imported Embedding from std.kesnet."
  },
  {
    "name": "weight",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Embedding",
    "field": true,
    "signature": "Embedding.weight : Variable",
    "parameters": [],
    "documentation": "Field weight of Embedding."
  },
  {
    "name": "numEmbeddings",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Embedding",
    "field": true,
    "signature": "Embedding.numEmbeddings : Int",
    "parameters": [],
    "documentation": "Field numEmbeddings of Embedding."
  },
  {
    "name": "embeddingDim",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "Embedding",
    "field": true,
    "signature": "Embedding.embeddingDim : Int",
    "parameters": [],
    "documentation": "Field embeddingDim of Embedding."
  },
  {
    "name": "Embedding",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "Embedding(numEmbeddings : Int, embeddingDim : Int) : Embedding",
    "parameters": [
      "numEmbeddings : Int",
      "embeddingDim : Int"
    ],
    "documentation": "Constructs a Embedding."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "Embedding",
    "signature": "Embedding.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of Embedding."
  },
  {
    "name": "MultiheadAttention",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "MultiheadAttention",
    "parameters": [],
    "documentation": "Imported MultiheadAttention from std.kesnet."
  },
  {
    "name": "query",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.query : Linear",
    "parameters": [],
    "documentation": "Field query of MultiheadAttention."
  },
  {
    "name": "key",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.key : Linear",
    "parameters": [],
    "documentation": "Field key of MultiheadAttention."
  },
  {
    "name": "value",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.value : Linear",
    "parameters": [],
    "documentation": "Field value of MultiheadAttention."
  },
  {
    "name": "output",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.output : Linear",
    "parameters": [],
    "documentation": "Field output of MultiheadAttention."
  },
  {
    "name": "embedDim",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.embedDim : Int",
    "parameters": [],
    "documentation": "Field embedDim of MultiheadAttention."
  },
  {
    "name": "heads",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.heads : Int",
    "parameters": [],
    "documentation": "Field heads of MultiheadAttention."
  },
  {
    "name": "headDim",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.headDim : Int",
    "parameters": [],
    "documentation": "Field headDim of MultiheadAttention."
  },
  {
    "name": "useBias",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MultiheadAttention",
    "field": true,
    "signature": "MultiheadAttention.useBias : Bool",
    "parameters": [],
    "documentation": "Field useBias of MultiheadAttention."
  },
  {
    "name": "MultiheadAttention",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "MultiheadAttention(embedDim : Int, heads : Int, bias : Bool = true) : MultiheadAttention",
    "parameters": [
      "embedDim : Int",
      "heads : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a MultiheadAttention."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "MultiheadAttention",
    "signature": "MultiheadAttention.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of MultiheadAttention."
  },
  {
    "name": "TransformerEncoderLayer",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "TransformerEncoderLayer",
    "parameters": [],
    "documentation": "Imported TransformerEncoderLayer from std.kesnet."
  },
  {
    "name": "attention",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.attention : MultiheadAttention",
    "parameters": [],
    "documentation": "Field attention of TransformerEncoderLayer."
  },
  {
    "name": "norm1",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.norm1 : LayerNorm",
    "parameters": [],
    "documentation": "Field norm1 of TransformerEncoderLayer."
  },
  {
    "name": "norm2",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.norm2 : LayerNorm",
    "parameters": [],
    "documentation": "Field norm2 of TransformerEncoderLayer."
  },
  {
    "name": "linear1",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.linear1 : Linear",
    "parameters": [],
    "documentation": "Field linear1 of TransformerEncoderLayer."
  },
  {
    "name": "linear2",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.linear2 : Linear",
    "parameters": [],
    "documentation": "Field linear2 of TransformerEncoderLayer."
  },
  {
    "name": "dropout",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.dropout : Dropout",
    "parameters": [],
    "documentation": "Field dropout of TransformerEncoderLayer."
  },
  {
    "name": "feedforwardDim",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoderLayer",
    "field": true,
    "signature": "TransformerEncoderLayer.feedforwardDim : Int",
    "parameters": [],
    "documentation": "Field feedforwardDim of TransformerEncoderLayer."
  },
  {
    "name": "TransformerEncoderLayer",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "TransformerEncoderLayer(embedDim : Int, heads : Int, feedforwardDim : Int = 0, dropout : double = 0.1) : TransformerEncoderLayer",
    "parameters": [
      "embedDim : Int",
      "heads : Int",
      "feedforwardDim : Int = 0",
      "dropout : double = 0.1"
    ],
    "documentation": "Constructs a TransformerEncoderLayer."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "TransformerEncoderLayer",
    "signature": "TransformerEncoderLayer.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of TransformerEncoderLayer."
  },
  {
    "name": "TransformerEncoder",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "TransformerEncoder",
    "parameters": [],
    "documentation": "Imported TransformerEncoder from std.kesnet."
  },
  {
    "name": "layers",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "TransformerEncoder",
    "field": true,
    "signature": "TransformerEncoder.layers : List[Box[dyn Network]]",
    "parameters": [],
    "documentation": "Field layers of TransformerEncoder."
  },
  {
    "name": "TransformerEncoder",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "TransformerEncoder(layers : List[Box[dyn Network]]) : TransformerEncoder",
    "parameters": [
      "layers : List[Box[dyn Network]]"
    ],
    "documentation": "Constructs a TransformerEncoder."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "TransformerEncoder",
    "signature": "TransformerEncoder.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of TransformerEncoder."
  },
  {
    "name": "LSTMCell",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "LSTMCell",
    "parameters": [],
    "documentation": "Imported LSTMCell from std.kesnet."
  },
  {
    "name": "inputInput",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.inputInput : Linear",
    "parameters": [],
    "documentation": "Field inputInput of LSTMCell."
  },
  {
    "name": "inputForget",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.inputForget : Linear",
    "parameters": [],
    "documentation": "Field inputForget of LSTMCell."
  },
  {
    "name": "inputCell",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.inputCell : Linear",
    "parameters": [],
    "documentation": "Field inputCell of LSTMCell."
  },
  {
    "name": "inputOutput",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.inputOutput : Linear",
    "parameters": [],
    "documentation": "Field inputOutput of LSTMCell."
  },
  {
    "name": "hiddenInput",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hiddenInput : Linear",
    "parameters": [],
    "documentation": "Field hiddenInput of LSTMCell."
  },
  {
    "name": "hiddenForget",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hiddenForget : Linear",
    "parameters": [],
    "documentation": "Field hiddenForget of LSTMCell."
  },
  {
    "name": "hiddenCell",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hiddenCell : Linear",
    "parameters": [],
    "documentation": "Field hiddenCell of LSTMCell."
  },
  {
    "name": "hiddenOutput",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hiddenOutput : Linear",
    "parameters": [],
    "documentation": "Field hiddenOutput of LSTMCell."
  },
  {
    "name": "hiddenSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hiddenSize : Int",
    "parameters": [],
    "documentation": "Field hiddenSize of LSTMCell."
  },
  {
    "name": "hasBias",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTMCell",
    "field": true,
    "signature": "LSTMCell.hasBias : Bool",
    "parameters": [],
    "documentation": "Field hasBias of LSTMCell."
  },
  {
    "name": "LSTMCell",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "LSTMCell(inputSize : Int, hiddenSize : Int, bias : Bool = true) : LSTMCell",
    "parameters": [
      "inputSize : Int",
      "hiddenSize : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a LSTMCell."
  },
  {
    "name": "step",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "LSTMCell",
    "signature": "LSTMCell.step(x : Variable, hidden : Variable, cell : Variable) : (Variable, Variable)",
    "parameters": [
      "x : Variable",
      "hidden : Variable",
      "cell : Variable"
    ],
    "documentation": "Member of LSTMCell."
  },
  {
    "name": "zeros",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "LSTMCell",
    "signature": "LSTMCell.zeros(batch : Int) : (Variable, Variable)",
    "parameters": [
      "batch : Int"
    ],
    "documentation": "Member of LSTMCell."
  },
  {
    "name": "LSTM",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "LSTM",
    "parameters": [],
    "documentation": "Imported LSTM from std.kesnet."
  },
  {
    "name": "cell",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTM",
    "field": true,
    "signature": "LSTM.cell : LSTMCell",
    "parameters": [],
    "documentation": "Field cell of LSTM."
  },
  {
    "name": "hiddenSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "LSTM",
    "field": true,
    "signature": "LSTM.hiddenSize : Int",
    "parameters": [],
    "documentation": "Field hiddenSize of LSTM."
  },
  {
    "name": "LSTM",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "LSTM(inputSize : Int, hiddenSize : Int, bias : Bool = true) : LSTM",
    "parameters": [
      "inputSize : Int",
      "hiddenSize : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a LSTM."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "LSTM",
    "signature": "LSTM.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of LSTM."
  },
  {
    "name": "GRUCell",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "GRUCell",
    "parameters": [],
    "documentation": "Imported GRUCell from std.kesnet."
  },
  {
    "name": "inputReset",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.inputReset : Linear",
    "parameters": [],
    "documentation": "Field inputReset of GRUCell."
  },
  {
    "name": "inputUpdate",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.inputUpdate : Linear",
    "parameters": [],
    "documentation": "Field inputUpdate of GRUCell."
  },
  {
    "name": "inputNew",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.inputNew : Linear",
    "parameters": [],
    "documentation": "Field inputNew of GRUCell."
  },
  {
    "name": "hiddenReset",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.hiddenReset : Linear",
    "parameters": [],
    "documentation": "Field hiddenReset of GRUCell."
  },
  {
    "name": "hiddenUpdate",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.hiddenUpdate : Linear",
    "parameters": [],
    "documentation": "Field hiddenUpdate of GRUCell."
  },
  {
    "name": "hiddenNew",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.hiddenNew : Linear",
    "parameters": [],
    "documentation": "Field hiddenNew of GRUCell."
  },
  {
    "name": "hiddenSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRUCell",
    "field": true,
    "signature": "GRUCell.hiddenSize : Int",
    "parameters": [],
    "documentation": "Field hiddenSize of GRUCell."
  },
  {
    "name": "GRUCell",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "GRUCell(inputSize : Int, hiddenSize : Int, bias : Bool = true) : GRUCell",
    "parameters": [
      "inputSize : Int",
      "hiddenSize : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a GRUCell."
  },
  {
    "name": "step",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "GRUCell",
    "signature": "GRUCell.step(x : Variable, hidden : Variable) : Variable",
    "parameters": [
      "x : Variable",
      "hidden : Variable"
    ],
    "documentation": "Member of GRUCell."
  },
  {
    "name": "zeros",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "GRUCell",
    "signature": "GRUCell.zeros(batch : Int) : Variable",
    "parameters": [
      "batch : Int"
    ],
    "documentation": "Member of GRUCell."
  },
  {
    "name": "GRU",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "GRU",
    "parameters": [],
    "documentation": "Imported GRU from std.kesnet."
  },
  {
    "name": "cell",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRU",
    "field": true,
    "signature": "GRU.cell : GRUCell",
    "parameters": [],
    "documentation": "Field cell of GRU."
  },
  {
    "name": "hiddenSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "GRU",
    "field": true,
    "signature": "GRU.hiddenSize : Int",
    "parameters": [],
    "documentation": "Field hiddenSize of GRU."
  },
  {
    "name": "GRU",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "GRU(inputSize : Int, hiddenSize : Int, bias : Bool = true) : GRU",
    "parameters": [
      "inputSize : Int",
      "hiddenSize : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a GRU."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "GRU",
    "signature": "GRU.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of GRU."
  },
  {
    "name": "RNNCell",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "RNNCell",
    "parameters": [],
    "documentation": "Imported RNNCell from std.kesnet."
  },
  {
    "name": "inputProjection",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "RNNCell",
    "field": true,
    "signature": "RNNCell.inputProjection : Linear",
    "parameters": [],
    "documentation": "Field inputProjection of RNNCell."
  },
  {
    "name": "hiddenProjection",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "RNNCell",
    "field": true,
    "signature": "RNNCell.hiddenProjection : Linear",
    "parameters": [],
    "documentation": "Field hiddenProjection of RNNCell."
  },
  {
    "name": "hiddenSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "RNNCell",
    "field": true,
    "signature": "RNNCell.hiddenSize : Int",
    "parameters": [],
    "documentation": "Field hiddenSize of RNNCell."
  },
  {
    "name": "RNNCell",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "RNNCell(inputSize : Int, hiddenSize : Int, bias : Bool = true) : RNNCell",
    "parameters": [
      "inputSize : Int",
      "hiddenSize : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a RNNCell."
  },
  {
    "name": "step",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "RNNCell",
    "signature": "RNNCell.step(x : Variable, hidden : Variable) : Variable",
    "parameters": [
      "x : Variable",
      "hidden : Variable"
    ],
    "documentation": "Member of RNNCell."
  },
  {
    "name": "zeros",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "RNNCell",
    "signature": "RNNCell.zeros(batch : Int) : Variable",
    "parameters": [
      "batch : Int"
    ],
    "documentation": "Member of RNNCell."
  },
  {
    "name": "RNN",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "RNN",
    "parameters": [],
    "documentation": "Imported RNN from std.kesnet."
  },
  {
    "name": "cell",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "RNN",
    "field": true,
    "signature": "RNN.cell : RNNCell",
    "parameters": [],
    "documentation": "Field cell of RNN."
  },
  {
    "name": "hiddenSize",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "RNN",
    "field": true,
    "signature": "RNN.hiddenSize : Int",
    "parameters": [],
    "documentation": "Field hiddenSize of RNN."
  },
  {
    "name": "RNN",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "RNN(inputSize : Int, hiddenSize : Int, bias : Bool = true) : RNN",
    "parameters": [
      "inputSize : Int",
      "hiddenSize : Int",
      "bias : Bool = true"
    ],
    "documentation": "Constructs a RNN."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "RNN",
    "signature": "RNN.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of RNN."
  },
  {
    "name": "cat",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "cat(values : List[Variable], axis : Int) : Variable",
    "parameters": [
      "values : List[Variable]",
      "axis : Int"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "stack",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "stack(values : List[Variable], axis : Int) : Variable",
    "parameters": [
      "values : List[Variable]",
      "axis : Int"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "saveParameters",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "saveParameters(module : Module, path : String) : void with Effects",
    "parameters": [
      "module : Module",
      "path : String"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "loadParameters",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "loadParameters(module : Module, path : String) : void with Effects",
    "parameters": [
      "module : Module",
      "path : String"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "accuracy",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "accuracy(logits : Variable, targets : Variable) : double",
    "parameters": [
      "logits : Variable",
      "targets : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "meanAbsoluteError",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "meanAbsoluteError(prediction : Variable, target : Variable) : double",
    "parameters": [
      "prediction : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "meanSquaredError",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "meanSquaredError(prediction : Variable, target : Variable) : double",
    "parameters": [
      "prediction : Variable",
      "target : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "xavierUniform",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "xavierUniform(weight : Variable) : void",
    "parameters": [
      "weight : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "kaimingUniform",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "kaimingUniform(weight : Variable) : void",
    "parameters": [
      "weight : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "normalInit",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "normalInit(weight : Variable, mean : double = 0.0, sigma : double = 0.01) : void",
    "parameters": [
      "weight : Variable",
      "mean : double = 0.0",
      "sigma : double = 0.01"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "zerosInit",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "zerosInit(weight : Variable) : void",
    "parameters": [
      "weight : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "onesInit",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "onesInit(weight : Variable) : void",
    "parameters": [
      "weight : Variable"
    ],
    "documentation": "Free function from std.kesnet."
  },
  {
    "name": "MLP",
    "module": "std.kesnet",
    "gated": true,
    "type": true,
    "signature": "MLP",
    "parameters": [],
    "documentation": "Imported MLP from std.kesnet."
  },
  {
    "name": "first",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MLP",
    "field": true,
    "signature": "MLP.first : Linear",
    "parameters": [],
    "documentation": "Field first of MLP."
  },
  {
    "name": "second",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MLP",
    "field": true,
    "signature": "MLP.second : Linear",
    "parameters": [],
    "documentation": "Field second of MLP."
  },
  {
    "name": "hidden",
    "module": "std.kesnet",
    "gated": true,
    "receiver": "MLP",
    "field": true,
    "signature": "MLP.hidden : Box[dyn Network]",
    "parameters": [],
    "documentation": "Field hidden of MLP."
  },
  {
    "name": "MLP",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "signature": "MLP(inputs : Int, hiddenSize : Int, outputs : Int, move activation : Box[dyn Network] = box(ReLU())) : MLP",
    "parameters": [
      "inputs : Int",
      "hiddenSize : Int",
      "outputs : Int",
      "move activation : Box[dyn Network] = box(ReLU())"
    ],
    "documentation": "Constructs a MLP."
  },
  {
    "name": "forward",
    "module": "std.kesnet",
    "gated": true,
    "named": true,
    "receiver": "MLP",
    "signature": "MLP.forward(x : Variable) : Variable",
    "parameters": [
      "x : Variable"
    ],
    "documentation": "Member of MLP."
  }
];

const names = [
  "Module",
  "registered",
  "trainingMode",
  "addParameter",
  "parameters",
  "parameterCount",
  "train",
  "eval",
  "zeroGrad",
  "forward",
  "summary",
  "Identity",
  "Linear",
  "weight",
  "biasValue",
  "hasBias",
  "inFeatures",
  "outFeatures",
  "ReLU",
  "LeakyReLU",
  "slope",
  "ELU",
  "alpha",
  "GELU",
  "SiLU",
  "Sigmoid",
  "Tanh",
  "Softmax",
  "axis",
  "LogSoftmax",
  "Flatten",
  "startDim",
  "Dropout",
  "probability",
  "Sequential",
  "layers",
  "ModuleList",
  "MSELoss",
  "reduction",
  "L1Loss",
  "CrossEntropyLoss",
  "NLLLoss",
  "BCEWithLogitsLoss",
  "relu",
  "sigmoid",
  "tanh",
  "gelu",
  "silu",
  "softmax",
  "logSoftmax",
  "linear",
  "mseLoss",
  "l1Loss",
  "crossEntropy",
  "nllLoss",
  "binaryCrossEntropy",
  "Conv2d",
  "inChannels",
  "outChannels",
  "kernelSize",
  "stride",
  "padding",
  "dilation",
  "groups",
  "MaxPool2d",
  "AvgPool2d",
  "AdaptiveAvgPool2d",
  "outHeight",
  "outWidth",
  "BatchNorm2d",
  "runningMean",
  "runningVar",
  "epsilon",
  "momentum",
  "affine",
  "BatchNorm1d",
  "inner",
  "LayerNorm",
  "GroupNorm",
  "numGroups",
  "numChannels",
  "Embedding",
  "numEmbeddings",
  "embeddingDim",
  "MultiheadAttention",
  "query",
  "key",
  "value",
  "output",
  "embedDim",
  "heads",
  "headDim",
  "useBias",
  "TransformerEncoderLayer",
  "attention",
  "norm1",
  "norm2",
  "linear1",
  "linear2",
  "dropout",
  "feedforwardDim",
  "TransformerEncoder",
  "LSTMCell",
  "inputInput",
  "inputForget",
  "inputCell",
  "inputOutput",
  "hiddenInput",
  "hiddenForget",
  "hiddenCell",
  "hiddenOutput",
  "hiddenSize",
  "step",
  "zeros",
  "LSTM",
  "cell",
  "GRUCell",
  "inputReset",
  "inputUpdate",
  "inputNew",
  "hiddenReset",
  "hiddenUpdate",
  "hiddenNew",
  "GRU",
  "RNNCell",
  "inputProjection",
  "hiddenProjection",
  "RNN",
  "cat",
  "stack",
  "saveParameters",
  "loadParameters",
  "accuracy",
  "meanAbsoluteError",
  "meanSquaredError",
  "xavierUniform",
  "kaimingUniform",
  "normalInit",
  "zerosInit",
  "onesInit",
  "MLP",
  "first",
  "second",
  "hidden"
];
const types = [
  "Module",
  "Identity",
  "Linear",
  "ReLU",
  "LeakyReLU",
  "ELU",
  "GELU",
  "SiLU",
  "Sigmoid",
  "Tanh",
  "Softmax",
  "LogSoftmax",
  "Flatten",
  "Dropout",
  "Sequential",
  "ModuleList",
  "MSELoss",
  "L1Loss",
  "CrossEntropyLoss",
  "NLLLoss",
  "BCEWithLogitsLoss",
  "Conv2d",
  "MaxPool2d",
  "AvgPool2d",
  "AdaptiveAvgPool2d",
  "BatchNorm2d",
  "BatchNorm1d",
  "LayerNorm",
  "GroupNorm",
  "Embedding",
  "MultiheadAttention",
  "TransformerEncoderLayer",
  "TransformerEncoder",
  "LSTMCell",
  "LSTM",
  "GRUCell",
  "GRU",
  "RNNCell",
  "RNN",
  "MLP"
];
const fieldTypes = new Map();
for (const entry of entries) {
  if (entry.field) fieldTypes.set(`${entry.receiver}.${entry.name}`, entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim());
}

const parents = {"Identity":"Module","Linear":"Module","ReLU":"Module","LeakyReLU":"Module","ELU":"Module","GELU":"Module","SiLU":"Module","Sigmoid":"Module","Tanh":"Module","Softmax":"Module","LogSoftmax":"Module","Flatten":"Module","Dropout":"Module","Sequential":"Module","ModuleList":"Module","Conv2d":"Module","MaxPool2d":"Module","AvgPool2d":"Module","AdaptiveAvgPool2d":"Module","BatchNorm2d":"Module","BatchNorm1d":"Module","LayerNorm":"Module","GroupNorm":"Module","Embedding":"Module","MultiheadAttention":"Module","TransformerEncoderLayer":"Module","TransformerEncoder":"Module","LSTMCell":"Module","LSTM":"Module","GRUCell":"Module","GRU":"Module","RNNCell":"Module","RNN":"Module","MLP":"Module"};

module.exports = { types, names, entries, fieldTypes, parents };
