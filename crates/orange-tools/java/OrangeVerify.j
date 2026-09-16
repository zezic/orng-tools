; SPDX-FileCopyrightText: 2026 Sergey Ukolov
; SPDX-License-Identifier: GPL-3.0-only
;
; Generated from OrangeVerify.java; see that file for what this is and how to
; regenerate it. Assembled at run time by krakatau2, so no JDK is needed to
; build or use this crate.
.version 61 0
.class public final super OrangeVerify
.super java/lang/Object

.method public <init> : ()V
    .code stack 1 locals 1
L0:     aload_0
L1:     invokespecial Method java/lang/Object <init> ()V
L4:     return
L5:
    .end code
.end method

.method public static main : ([Ljava/lang/String;)V
    .code stack 3 locals 6
L0:     ldc Class OrangeVerify
L2:     invokevirtual Method java/lang/Class getClassLoader ()Ljava/lang/ClassLoader;
L5:     astore_1
L6:     aload_0
L7:     astore_2
L8:     aload_2
L9:     arraylength
L10:    istore_3
L11:    iconst_0
L12:    istore 4

        .stack full
            locals Object [Ljava/lang/String; Object java/lang/ClassLoader Object [Ljava/lang/String; Integer Integer
            stack
        .end stack
L14:    iload 4
L16:    iload_3
L17:    if_icmpge L40
L20:    aload_2
L21:    iload 4
L23:    aaload
L24:    astore 5
L26:    aload 5
L28:    iconst_1
L29:    aload_1
L30:    invokestatic Method java/lang/Class forName (Ljava/lang/String;ZLjava/lang/ClassLoader;)Ljava/lang/Class;
L33:    pop
L34:    iinc 4 1
L37:    goto L14

        .stack chop 3
L40:    return
L41:
    .end code
    .exceptions java/lang/Throwable
.end method
.end class
