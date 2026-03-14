package jvankomrt;

import java.io.PrintStream;

public class JVMInit {
    public static void init() {
        PrintStream printStream = JVankoMPrintStream.construct();
        System.setOut(printStream);
        System.setErr(printStream);
    }
}
