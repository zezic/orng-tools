; SPDX-FileCopyrightText: 2026 Sergey Ukolov
; SPDX-License-Identifier: GPL-3.0-only
;
; Generated from OrngRegistry.java; see that file for what this is and how to
; regenerate it. Assembled at run time by krakatau2, so no JDK is needed to
; build or use this crate.
;
; The orng/placeholder names below are rewritten in the constant pool at patch
; time, to whatever the Bitwig build being prepared calls them. Everything else
; is java.base and stays as it is.
.version 61 0
.class public final super OrngRegistry
.super java/lang/Object
.field private static final UUID_COLUMN I = 0
.field private static final KIND_COLUMN I = 1
.field private static final NAME_COLUMN I = 2
.field private static final PATH_COLUMN I = 3
.field private static final COLUMNS I = 4

.method private <init> : ()V
    .code stack 1 locals 1
L0:     aload_0
L1:     invokespecial Method java/lang/Object <init> ()V
L4:     return
L5:
    .end code
.end method

.method public static install : ()V
    .code stack 5 locals 3
        .catch java/lang/Throwable from L0 to L60 using L63
L0:     invokestatic Method OrngRegistry entries ()Ljava/util/List;
L3:     invokeinterface InterfaceMethod java/util/List iterator ()Ljava/util/Iterator; 1
L8:     astore_0

        .stack append Object java/util/Iterator
L9:     aload_0
L10:    invokeinterface InterfaceMethod java/util/Iterator hasNext ()Z 1
L15:    ifeq L60
L18:    aload_0
L19:    invokeinterface InterfaceMethod java/util/Iterator next ()Ljava/lang/Object; 1
L24:    checkcast [Ljava/lang/String;
L27:    astore_1
L28:    ldc Class orng/placeholder/Kind
L30:    aload_1
L31:    iconst_1
L32:    aaload
L33:    invokestatic Method java/lang/Enum valueOf (Ljava/lang/Class;Ljava/lang/String;)Ljava/lang/Enum;
L36:    checkcast orng/placeholder/Kind
L39:    astore_2
L40:    aload_1
L41:    iconst_0
L42:    aaload
L43:    invokestatic Method java/util/UUID fromString (Ljava/lang/String;)Ljava/util/UUID;
L46:    aload_1
L47:    iconst_2
L48:    aaload
L49:    aload_2
L50:    aload_1
L51:    iconst_3
L52:    aaload
L53:    iconst_0
L54:    invokestatic Method orng/placeholder/Registry register (Ljava/util/UUID;Ljava/lang/String;Lorng/placeholder/Kind;Ljava/lang/String;Z)V
L57:    goto L9

        .stack chop 1
L60:    goto L68

        .stack stack_1 Object java/lang/Throwable
L63:    astore_0
L64:    aload_0
L65:    invokestatic Method OrngRegistry report (Ljava/lang/Throwable;)V

        .stack same
L68:    return
L69:
    .end code
.end method

.method public static grant : (Ljava/util/HashMap;Ljava/util/HashMap;Ljava/util/HashMap;)V
    .code stack 5 locals 6
        .catch java/lang/Throwable from L0 to L89 using L92
L0:     invokestatic Method OrngRegistry entries ()Ljava/util/List;
L3:     invokeinterface InterfaceMethod java/util/List iterator ()Ljava/util/Iterator; 1
L8:     astore_3

        .stack append Object java/util/Iterator
L9:     aload_3
L10:    invokeinterface InterfaceMethod java/util/Iterator hasNext ()Z 1
L15:    ifeq L89
L18:    aload_3
L19:    invokeinterface InterfaceMethod java/util/Iterator next ()Ljava/lang/Object; 1
L24:    checkcast [Ljava/lang/String;
L27:    astore 4
L29:    aload 4
L31:    iconst_0
L32:    aaload
L33:    invokestatic Method java/util/UUID fromString (Ljava/lang/String;)Ljava/util/UUID;
L36:    astore 5
L38:    aload_0
L39:    aload 5
L41:    new orng/placeholder/Grant
L44:    dup
L45:    aload 5
L47:    invokespecial Method orng/placeholder/Grant <init> (Ljava/util/UUID;)V
L50:    invokevirtual Method java/util/HashMap put (Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;
L53:    pop
L54:    aload_1
L55:    aload 5
L57:    new orng/placeholder/Grant
L60:    dup
L61:    aload 5
L63:    invokespecial Method orng/placeholder/Grant <init> (Ljava/util/UUID;)V
L66:    invokevirtual Method java/util/HashMap put (Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;
L69:    pop
L70:    aload_2
L71:    aload 5
L73:    new orng/placeholder/Grant
L76:    dup
L77:    aload 5
L79:    invokespecial Method orng/placeholder/Grant <init> (Ljava/util/UUID;)V
L82:    invokevirtual Method java/util/HashMap put (Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;
L85:    pop
L86:    goto L9

        .stack chop 1
L89:    goto L97

        .stack stack_1 Object java/lang/Throwable
L92:    astore_3
L93:    aload_3
L94:    invokestatic Method OrngRegistry report (Ljava/lang/Throwable;)V

        .stack same
L97:    return
L98:
    .end code
.end method

.method private static entries : ()Ljava/util/List;
    .code stack 5 locals 5
L0:     ldc "user.home"
L2:     invokestatic Method java/lang/System getProperty (Ljava/lang/String;)Ljava/lang/String;
L5:     iconst_2
L6:     anewarray java/lang/String
L9:     dup
L10:    iconst_0
L11:    ldc ".orng"
L13:    aastore
L14:    dup
L15:    iconst_1
L16:    ldc "entries.tsv"
L18:    aastore
L19:    invokestatic InterfaceMethod java/nio/file/Path of (Ljava/lang/String;[Ljava/lang/String;)Ljava/nio/file/Path;
L22:    astore_0
L23:    new java/util/ArrayList
L26:    dup
L27:    invokespecial Method java/util/ArrayList <init> ()V
L30:    astore_1
L31:    aload_0
L32:    invokestatic Method java/nio/file/Files isReadable (Ljava/nio/file/Path;)Z
L35:    ifne L40
L38:    aload_1
L39:    areturn

        .stack append Object java/nio/file/Path Object java/util/List
L40:    aload_0
L41:    invokestatic Method java/nio/file/Files readAllLines (Ljava/nio/file/Path;)Ljava/util/List;
L44:    invokeinterface InterfaceMethod java/util/List iterator ()Ljava/util/Iterator; 1
L49:    astore_2

        .stack append Object java/util/Iterator
L50:    aload_2
L51:    invokeinterface InterfaceMethod java/util/Iterator hasNext ()Z 1
L56:    ifeq L116
L59:    aload_2
L60:    invokeinterface InterfaceMethod java/util/Iterator next ()Ljava/lang/Object; 1
L65:    checkcast java/lang/String
L68:    astore_3
L69:    aload_3
L70:    invokevirtual Method java/lang/String isEmpty ()Z
L73:    ifne L50
L76:    aload_3
L77:    iconst_0
L78:    invokevirtual Method java/lang/String charAt (I)C
L81:    bipush 35
L83:    if_icmpne L89
L86:    goto L50

        .stack append Object java/lang/String
L89:    aload_3
L90:    ldc "\u0009"
L92:    invokevirtual Method java/lang/String split (Ljava/lang/String;)[Ljava/lang/String;
L95:    astore 4
L97:    aload 4
L99:    arraylength
L100:   iconst_4
L101:   if_icmplt L113
L104:   aload_1
L105:   aload 4
L107:   invokeinterface InterfaceMethod java/util/List add (Ljava/lang/Object;)Z 2
L112:   pop

        .stack chop 1
L113:   goto L50

        .stack chop 1
L116:   aload_1
L117:   areturn
L118:
    .end code
    .exceptions java/lang/Exception
.end method

.method private static report : (Ljava/lang/Throwable;)V
    .code stack 2 locals 1
L0:     getstatic Field java/lang/System err Ljava/io/PrintStream;
L3:     aload_0
L4:     invokestatic Method java/lang/String valueOf (Ljava/lang/Object;)Ljava/lang/String;
L7:     invokedynamic [_125]
L12:    invokevirtual Method java/io/PrintStream println (Ljava/lang/String;)V
L15:    return
L16:
    .end code
.end method
.bootstrapmethods
.innerclasses
    java/lang/invoke/MethodHandles$Lookup java/lang/invoke/MethodHandles Lookup public static final
.end innerclasses
.const [_125] = InvokeDynamic invokeStatic Method java/lang/invoke/StringConcatFactory makeConcatWithConstants (Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/String;[Ljava/lang/Object;)Ljava/lang/invoke/CallSite; String "orng-registry: could not apply the entry list: \u0001" : makeConcatWithConstants (Ljava/lang/String;)Ljava/lang/String;
.end class
