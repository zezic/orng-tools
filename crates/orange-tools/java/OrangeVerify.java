// Authoring source for OrangeVerify.j, which is what the crate actually uses.
//
// This class never enters an installation. It is written to a temporary
// directory, put on the classpath ahead of a patched bitwig.jar, and asked to
// load the classes preparation edited. Loading links them, and linking runs the
// JVM's own verifier, so a patch that does not verify is rejected before the
// installation is touched.
//
// Class names are arguments rather than constants because they are obfuscated
// and change on every Bitwig release. That keeps this class fixed: nothing here
// is retargeted per build.
//
// Regenerate the assembly after editing:
//
//   javac --release 17 -d <tmp> OrangeVerify.java
//   krak2 dis --out OrangeVerify.j <tmp>/OrangeVerify.class
//
// then delete the .linenumbertable and .sourcefile lines, which only name this
// file and are not worth carrying into the classpath.
public final class OrangeVerify {
    public static void main(String[] args) throws Throwable {
        ClassLoader loader = OrangeVerify.class.getClassLoader();
        for (String name : args) {
            Class.forName(name, true, loader);
        }
    }
}
