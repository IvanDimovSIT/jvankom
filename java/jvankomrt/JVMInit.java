package jvankomrt;

import java.io.PrintStream;

public class JVMInit {
    public static void init() {
        loadClasses();
        PrintStream printStream = JVankoMPrintStream.construct();
        System.setOut(printStream);
        System.setErr(printStream);
    }

    private static void loadClasses() {
        new String();
        Object.class.getClass();
    }
}
