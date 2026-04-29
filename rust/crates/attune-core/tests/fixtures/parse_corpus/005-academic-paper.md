# Attention Is All You Need: A Brief Review

## Abstract

We propose the Transformer, a model architecture eschewing recurrence and instead relying entirely on an attention mechanism to draw global dependencies between input and output. Experiments on machine translation tasks show these models to be superior in quality while being more parallelizable.

## 1 Introduction

Recurrent neural networks, long short-term memory, and gated recurrent neural networks in particular, have been firmly established as state of the art approaches in sequence modeling and transduction problems such as language modeling and machine translation.

## 2 Background

The goal of reducing sequential computation also forms the foundation of the Extended Neural GPU, ByteNet, and ConvS2S, all of which use convolutional neural networks as basic building block.

### 2.1 Self-Attention

Self-attention, sometimes called intra-attention, is an attention mechanism relating different positions of a single sequence in order to compute a representation of the sequence. Self-attention has been used successfully in a variety of tasks.

### 2.2 End-to-End Networks

End-to-end memory networks are based on a recurrent attention mechanism instead of sequence-aligned recurrence and have been shown to perform well on simple-language question answering and language modeling tasks.

## 3 Model Architecture

Most competitive neural sequence transduction models have an encoder-decoder structure. The encoder maps an input sequence of symbol representations to a sequence of continuous representations.
