'use strict';

// Generated from src/kesnet_optim.kes by tools/export_kesnet_metadata.js.
const entries = [
  {
    "name": "Optimizer",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "Optimizer",
    "parameters": [],
    "documentation": "Imported Optimizer from std.kesnet.optim."
  },
  {
    "name": "parameters",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Optimizer",
    "field": true,
    "signature": "Optimizer.parameters : List[Variable]",
    "parameters": [],
    "documentation": "Field parameters of Optimizer."
  },
  {
    "name": "learningRate",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Optimizer",
    "field": true,
    "signature": "Optimizer.learningRate : double",
    "parameters": [],
    "documentation": "Field learningRate of Optimizer."
  },
  {
    "name": "Optimizer",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "Optimizer(parameters : List[Variable], learningRate : double = 0.01) : Optimizer",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.01"
    ],
    "documentation": "Constructs a Optimizer."
  },
  {
    "name": "zeroGrad",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Optimizer",
    "signature": "Optimizer.zeroGrad() : void",
    "parameters": [],
    "documentation": "Member of Optimizer."
  },
  {
    "name": "setLearningRate",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Optimizer",
    "signature": "Optimizer.setLearningRate(value : double) : void",
    "parameters": [
      "value : double"
    ],
    "documentation": "Member of Optimizer."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Optimizer",
    "signature": "Optimizer.step() : void",
    "parameters": [],
    "documentation": "Member of Optimizer."
  },
  {
    "name": "parameterCount",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Optimizer",
    "signature": "Optimizer.parameterCount() : Int",
    "parameters": [],
    "documentation": "Member of Optimizer."
  },
  {
    "name": "SGD",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "SGD",
    "parameters": [],
    "documentation": "Imported SGD from std.kesnet.optim."
  },
  {
    "name": "momentum",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "SGD",
    "field": true,
    "signature": "SGD.momentum : double",
    "parameters": [],
    "documentation": "Field momentum of SGD."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "SGD",
    "field": true,
    "signature": "SGD.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of SGD."
  },
  {
    "name": "nesterov",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "SGD",
    "field": true,
    "signature": "SGD.nesterov : Bool",
    "parameters": [],
    "documentation": "Field nesterov of SGD."
  },
  {
    "name": "velocities",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "SGD",
    "field": true,
    "signature": "SGD.velocities : Array[Variable]",
    "parameters": [],
    "documentation": "Field velocities of SGD."
  },
  {
    "name": "SGD",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "SGD(parameters : List[Variable], learningRate : double = 0.01, momentum : double = 0.0, weightDecay : double = 0.0, nesterov : Bool = false) : SGD",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.01",
      "momentum : double = 0.0",
      "weightDecay : double = 0.0",
      "nesterov : Bool = false"
    ],
    "documentation": "Constructs a SGD."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "SGD",
    "signature": "SGD.step() : void",
    "parameters": [],
    "documentation": "Member of SGD."
  },
  {
    "name": "Adam",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "Adam",
    "parameters": [],
    "documentation": "Imported Adam from std.kesnet.optim."
  },
  {
    "name": "beta1",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.beta1 : double",
    "parameters": [],
    "documentation": "Field beta1 of Adam."
  },
  {
    "name": "beta2",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.beta2 : double",
    "parameters": [],
    "documentation": "Field beta2 of Adam."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of Adam."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of Adam."
  },
  {
    "name": "firstMoments",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.firstMoments : Array[Variable]",
    "parameters": [],
    "documentation": "Field firstMoments of Adam."
  },
  {
    "name": "secondMoments",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.secondMoments : Array[Variable]",
    "parameters": [],
    "documentation": "Field secondMoments of Adam."
  },
  {
    "name": "steps",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adam",
    "field": true,
    "signature": "Adam.steps : Int",
    "parameters": [],
    "documentation": "Field steps of Adam."
  },
  {
    "name": "Adam",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "Adam(parameters : List[Variable], learningRate : double = 0.001, beta1 : double = 0.9, beta2 : double = 0.999, epsilon : double = 0.00000001, weightDecay : double = 0.0) : Adam",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.001",
      "beta1 : double = 0.9",
      "beta2 : double = 0.999",
      "epsilon : double = 0.00000001",
      "weightDecay : double = 0.0"
    ],
    "documentation": "Constructs a Adam."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Adam",
    "signature": "Adam.step() : void",
    "parameters": [],
    "documentation": "Member of Adam."
  },
  {
    "name": "AdamW",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "AdamW",
    "parameters": [],
    "documentation": "Imported AdamW from std.kesnet.optim."
  },
  {
    "name": "beta1",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.beta1 : double",
    "parameters": [],
    "documentation": "Field beta1 of AdamW."
  },
  {
    "name": "beta2",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.beta2 : double",
    "parameters": [],
    "documentation": "Field beta2 of AdamW."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of AdamW."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of AdamW."
  },
  {
    "name": "firstMoments",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.firstMoments : Array[Variable]",
    "parameters": [],
    "documentation": "Field firstMoments of AdamW."
  },
  {
    "name": "secondMoments",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.secondMoments : Array[Variable]",
    "parameters": [],
    "documentation": "Field secondMoments of AdamW."
  },
  {
    "name": "steps",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "AdamW",
    "field": true,
    "signature": "AdamW.steps : Int",
    "parameters": [],
    "documentation": "Field steps of AdamW."
  },
  {
    "name": "AdamW",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "AdamW(parameters : List[Variable], learningRate : double = 0.001, beta1 : double = 0.9, beta2 : double = 0.999, epsilon : double = 0.00000001, weightDecay : double = 0.01) : AdamW",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.001",
      "beta1 : double = 0.9",
      "beta2 : double = 0.999",
      "epsilon : double = 0.00000001",
      "weightDecay : double = 0.01"
    ],
    "documentation": "Constructs a AdamW."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "AdamW",
    "signature": "AdamW.step() : void",
    "parameters": [],
    "documentation": "Member of AdamW."
  },
  {
    "name": "RMSprop",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "RMSprop",
    "parameters": [],
    "documentation": "Imported RMSprop from std.kesnet.optim."
  },
  {
    "name": "alpha",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "RMSprop",
    "field": true,
    "signature": "RMSprop.alpha : double",
    "parameters": [],
    "documentation": "Field alpha of RMSprop."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "RMSprop",
    "field": true,
    "signature": "RMSprop.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of RMSprop."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "RMSprop",
    "field": true,
    "signature": "RMSprop.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of RMSprop."
  },
  {
    "name": "momentum",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "RMSprop",
    "field": true,
    "signature": "RMSprop.momentum : double",
    "parameters": [],
    "documentation": "Field momentum of RMSprop."
  },
  {
    "name": "squares",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "RMSprop",
    "field": true,
    "signature": "RMSprop.squares : Array[Variable]",
    "parameters": [],
    "documentation": "Field squares of RMSprop."
  },
  {
    "name": "velocities",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "RMSprop",
    "field": true,
    "signature": "RMSprop.velocities : Array[Variable]",
    "parameters": [],
    "documentation": "Field velocities of RMSprop."
  },
  {
    "name": "RMSprop",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "RMSprop(parameters : List[Variable], learningRate : double = 0.01, alpha : double = 0.99, epsilon : double = 0.00000001, weightDecay : double = 0.0, momentum : double = 0.0) : RMSprop",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.01",
      "alpha : double = 0.99",
      "epsilon : double = 0.00000001",
      "weightDecay : double = 0.0",
      "momentum : double = 0.0"
    ],
    "documentation": "Constructs a RMSprop."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "RMSprop",
    "signature": "RMSprop.step() : void",
    "parameters": [],
    "documentation": "Member of RMSprop."
  },
  {
    "name": "Adagrad",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "Adagrad",
    "parameters": [],
    "documentation": "Imported Adagrad from std.kesnet.optim."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adagrad",
    "field": true,
    "signature": "Adagrad.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of Adagrad."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adagrad",
    "field": true,
    "signature": "Adagrad.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of Adagrad."
  },
  {
    "name": "squares",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adagrad",
    "field": true,
    "signature": "Adagrad.squares : Array[Variable]",
    "parameters": [],
    "documentation": "Field squares of Adagrad."
  },
  {
    "name": "Adagrad",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "Adagrad(parameters : List[Variable], learningRate : double = 0.01, epsilon : double = 0.00000001, weightDecay : double = 0.0) : Adagrad",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.01",
      "epsilon : double = 0.00000001",
      "weightDecay : double = 0.0"
    ],
    "documentation": "Constructs a Adagrad."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Adagrad",
    "signature": "Adagrad.step() : void",
    "parameters": [],
    "documentation": "Member of Adagrad."
  },
  {
    "name": "Adadelta",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "Adadelta",
    "parameters": [],
    "documentation": "Imported Adadelta from std.kesnet.optim."
  },
  {
    "name": "rho",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adadelta",
    "field": true,
    "signature": "Adadelta.rho : double",
    "parameters": [],
    "documentation": "Field rho of Adadelta."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adadelta",
    "field": true,
    "signature": "Adadelta.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of Adadelta."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adadelta",
    "field": true,
    "signature": "Adadelta.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of Adadelta."
  },
  {
    "name": "gradientSquares",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adadelta",
    "field": true,
    "signature": "Adadelta.gradientSquares : Array[Variable]",
    "parameters": [],
    "documentation": "Field gradientSquares of Adadelta."
  },
  {
    "name": "updateSquares",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adadelta",
    "field": true,
    "signature": "Adadelta.updateSquares : Array[Variable]",
    "parameters": [],
    "documentation": "Field updateSquares of Adadelta."
  },
  {
    "name": "Adadelta",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "Adadelta(parameters : List[Variable], learningRate : double = 1.0, rho : double = 0.95, epsilon : double = 0.000001, weightDecay : double = 0.0) : Adadelta",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 1.0",
      "rho : double = 0.95",
      "epsilon : double = 0.000001",
      "weightDecay : double = 0.0"
    ],
    "documentation": "Constructs a Adadelta."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Adadelta",
    "signature": "Adadelta.step() : void",
    "parameters": [],
    "documentation": "Member of Adadelta."
  },
  {
    "name": "Adamax",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "Adamax",
    "parameters": [],
    "documentation": "Imported Adamax from std.kesnet.optim."
  },
  {
    "name": "beta1",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.beta1 : double",
    "parameters": [],
    "documentation": "Field beta1 of Adamax."
  },
  {
    "name": "beta2",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.beta2 : double",
    "parameters": [],
    "documentation": "Field beta2 of Adamax."
  },
  {
    "name": "epsilon",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.epsilon : double",
    "parameters": [],
    "documentation": "Field epsilon of Adamax."
  },
  {
    "name": "weightDecay",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.weightDecay : double",
    "parameters": [],
    "documentation": "Field weightDecay of Adamax."
  },
  {
    "name": "firstMoments",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.firstMoments : Array[Variable]",
    "parameters": [],
    "documentation": "Field firstMoments of Adamax."
  },
  {
    "name": "secondMoments",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.secondMoments : Array[Variable]",
    "parameters": [],
    "documentation": "Field secondMoments of Adamax."
  },
  {
    "name": "steps",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "Adamax",
    "field": true,
    "signature": "Adamax.steps : Int",
    "parameters": [],
    "documentation": "Field steps of Adamax."
  },
  {
    "name": "Adamax",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "Adamax(parameters : List[Variable], learningRate : double = 0.002, beta1 : double = 0.9, beta2 : double = 0.999, epsilon : double = 0.00000001, weightDecay : double = 0.0) : Adamax",
    "parameters": [
      "parameters : List[Variable]",
      "learningRate : double = 0.002",
      "beta1 : double = 0.9",
      "beta2 : double = 0.999",
      "epsilon : double = 0.00000001",
      "weightDecay : double = 0.0"
    ],
    "documentation": "Constructs a Adamax."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "Adamax",
    "signature": "Adamax.step() : void",
    "parameters": [],
    "documentation": "Member of Adamax."
  },
  {
    "name": "StepLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "StepLR",
    "parameters": [],
    "documentation": "Imported StepLR from std.kesnet.optim."
  },
  {
    "name": "stepSize",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "StepLR",
    "field": true,
    "signature": "StepLR.stepSize : Int",
    "parameters": [],
    "documentation": "Field stepSize of StepLR."
  },
  {
    "name": "gamma",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "StepLR",
    "field": true,
    "signature": "StepLR.gamma : double",
    "parameters": [],
    "documentation": "Field gamma of StepLR."
  },
  {
    "name": "completed",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "StepLR",
    "field": true,
    "signature": "StepLR.completed : Int",
    "parameters": [],
    "documentation": "Field completed of StepLR."
  },
  {
    "name": "StepLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "StepLR(stepSize : Int, gamma : double = 0.1) : StepLR",
    "parameters": [
      "stepSize : Int",
      "gamma : double = 0.1"
    ],
    "documentation": "Constructs a StepLR."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "StepLR",
    "signature": "StepLR.step(mut optimizer : Optimizer) : void",
    "parameters": [
      "mut optimizer : Optimizer"
    ],
    "documentation": "Member of StepLR."
  },
  {
    "name": "lastLearningRate",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "StepLR",
    "signature": "StepLR.lastLearningRate(optimizer : Optimizer) : double",
    "parameters": [
      "optimizer : Optimizer"
    ],
    "documentation": "Member of StepLR."
  },
  {
    "name": "MultiStepLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "MultiStepLR",
    "parameters": [],
    "documentation": "Imported MultiStepLR from std.kesnet.optim."
  },
  {
    "name": "milestones",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "MultiStepLR",
    "field": true,
    "signature": "MultiStepLR.milestones : List[Int]",
    "parameters": [],
    "documentation": "Field milestones of MultiStepLR."
  },
  {
    "name": "gamma",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "MultiStepLR",
    "field": true,
    "signature": "MultiStepLR.gamma : double",
    "parameters": [],
    "documentation": "Field gamma of MultiStepLR."
  },
  {
    "name": "completed",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "MultiStepLR",
    "field": true,
    "signature": "MultiStepLR.completed : Int",
    "parameters": [],
    "documentation": "Field completed of MultiStepLR."
  },
  {
    "name": "MultiStepLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "MultiStepLR(milestones : List[Int], gamma : double = 0.1) : MultiStepLR",
    "parameters": [
      "milestones : List[Int]",
      "gamma : double = 0.1"
    ],
    "documentation": "Constructs a MultiStepLR."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "MultiStepLR",
    "signature": "MultiStepLR.step(mut optimizer : Optimizer) : void",
    "parameters": [
      "mut optimizer : Optimizer"
    ],
    "documentation": "Member of MultiStepLR."
  },
  {
    "name": "ExponentialLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "ExponentialLR",
    "parameters": [],
    "documentation": "Imported ExponentialLR from std.kesnet.optim."
  },
  {
    "name": "gamma",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "ExponentialLR",
    "field": true,
    "signature": "ExponentialLR.gamma : double",
    "parameters": [],
    "documentation": "Field gamma of ExponentialLR."
  },
  {
    "name": "ExponentialLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "ExponentialLR(gamma : double) : ExponentialLR",
    "parameters": [
      "gamma : double"
    ],
    "documentation": "Constructs a ExponentialLR."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "ExponentialLR",
    "signature": "ExponentialLR.step(mut optimizer : Optimizer) : void",
    "parameters": [
      "mut optimizer : Optimizer"
    ],
    "documentation": "Member of ExponentialLR."
  },
  {
    "name": "CosineAnnealingLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "CosineAnnealingLR",
    "parameters": [],
    "documentation": "Imported CosineAnnealingLR from std.kesnet.optim."
  },
  {
    "name": "totalSteps",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "CosineAnnealingLR",
    "field": true,
    "signature": "CosineAnnealingLR.totalSteps : Int",
    "parameters": [],
    "documentation": "Field totalSteps of CosineAnnealingLR."
  },
  {
    "name": "minimum",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "CosineAnnealingLR",
    "field": true,
    "signature": "CosineAnnealingLR.minimum : double",
    "parameters": [],
    "documentation": "Field minimum of CosineAnnealingLR."
  },
  {
    "name": "initial",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "CosineAnnealingLR",
    "field": true,
    "signature": "CosineAnnealingLR.initial : double",
    "parameters": [],
    "documentation": "Field initial of CosineAnnealingLR."
  },
  {
    "name": "completed",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "CosineAnnealingLR",
    "field": true,
    "signature": "CosineAnnealingLR.completed : Int",
    "parameters": [],
    "documentation": "Field completed of CosineAnnealingLR."
  },
  {
    "name": "CosineAnnealingLR",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "CosineAnnealingLR(totalSteps : Int, initial : double, minimum : double = 0.0) : CosineAnnealingLR",
    "parameters": [
      "totalSteps : Int",
      "initial : double",
      "minimum : double = 0.0"
    ],
    "documentation": "Constructs a CosineAnnealingLR."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "CosineAnnealingLR",
    "signature": "CosineAnnealingLR.step(mut optimizer : Optimizer) : void",
    "parameters": [
      "mut optimizer : Optimizer"
    ],
    "documentation": "Member of CosineAnnealingLR."
  },
  {
    "name": "ReduceLROnPlateau",
    "module": "std.kesnet.optim",
    "gated": true,
    "type": true,
    "signature": "ReduceLROnPlateau",
    "parameters": [],
    "documentation": "Imported ReduceLROnPlateau from std.kesnet.optim."
  },
  {
    "name": "factor",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "ReduceLROnPlateau",
    "field": true,
    "signature": "ReduceLROnPlateau.factor : double",
    "parameters": [],
    "documentation": "Field factor of ReduceLROnPlateau."
  },
  {
    "name": "patience",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "ReduceLROnPlateau",
    "field": true,
    "signature": "ReduceLROnPlateau.patience : Int",
    "parameters": [],
    "documentation": "Field patience of ReduceLROnPlateau."
  },
  {
    "name": "minimum",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "ReduceLROnPlateau",
    "field": true,
    "signature": "ReduceLROnPlateau.minimum : double",
    "parameters": [],
    "documentation": "Field minimum of ReduceLROnPlateau."
  },
  {
    "name": "best",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "ReduceLROnPlateau",
    "field": true,
    "signature": "ReduceLROnPlateau.best : double",
    "parameters": [],
    "documentation": "Field best of ReduceLROnPlateau."
  },
  {
    "name": "waiting",
    "module": "std.kesnet.optim",
    "gated": true,
    "receiver": "ReduceLROnPlateau",
    "field": true,
    "signature": "ReduceLROnPlateau.waiting : Int",
    "parameters": [],
    "documentation": "Field waiting of ReduceLROnPlateau."
  },
  {
    "name": "ReduceLROnPlateau",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "ReduceLROnPlateau(factor : double = 0.1, patience : Int = 10, minimum : double = 0.000001) : ReduceLROnPlateau",
    "parameters": [
      "factor : double = 0.1",
      "patience : Int = 10",
      "minimum : double = 0.000001"
    ],
    "documentation": "Constructs a ReduceLROnPlateau."
  },
  {
    "name": "step",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "receiver": "ReduceLROnPlateau",
    "signature": "ReduceLROnPlateau.step(mut optimizer : Optimizer, metric : double) : void",
    "parameters": [
      "mut optimizer : Optimizer",
      "metric : double"
    ],
    "documentation": "Member of ReduceLROnPlateau."
  },
  {
    "name": "warmupRate",
    "module": "std.kesnet.optim",
    "gated": true,
    "named": true,
    "signature": "warmupRate(base : double, step : Int, warmupSteps : Int) : double",
    "parameters": [
      "base : double",
      "step : Int",
      "warmupSteps : Int"
    ],
    "documentation": "Free function from std.kesnet.optim."
  }
];

const names = [
  "Optimizer",
  "parameters",
  "learningRate",
  "zeroGrad",
  "setLearningRate",
  "step",
  "parameterCount",
  "SGD",
  "momentum",
  "weightDecay",
  "nesterov",
  "velocities",
  "Adam",
  "beta1",
  "beta2",
  "epsilon",
  "firstMoments",
  "secondMoments",
  "steps",
  "AdamW",
  "RMSprop",
  "alpha",
  "squares",
  "Adagrad",
  "Adadelta",
  "rho",
  "gradientSquares",
  "updateSquares",
  "Adamax",
  "StepLR",
  "stepSize",
  "gamma",
  "completed",
  "lastLearningRate",
  "MultiStepLR",
  "milestones",
  "ExponentialLR",
  "CosineAnnealingLR",
  "totalSteps",
  "minimum",
  "initial",
  "ReduceLROnPlateau",
  "factor",
  "patience",
  "best",
  "waiting",
  "warmupRate"
];
const types = [
  "Optimizer",
  "SGD",
  "Adam",
  "AdamW",
  "RMSprop",
  "Adagrad",
  "Adadelta",
  "Adamax",
  "StepLR",
  "MultiStepLR",
  "ExponentialLR",
  "CosineAnnealingLR",
  "ReduceLROnPlateau"
];
const fieldTypes = new Map();
for (const entry of entries) {
  if (entry.field) fieldTypes.set(`${entry.receiver}.${entry.name}`, entry.signature.slice(entry.signature.lastIndexOf(':') + 1).trim());
}

const parents = {"SGD":"Optimizer","Adam":"Optimizer","AdamW":"Optimizer","RMSprop":"Optimizer","Adagrad":"Optimizer","Adadelta":"Optimizer","Adamax":"Optimizer"};

module.exports = { types, names, entries, fieldTypes, parents };
