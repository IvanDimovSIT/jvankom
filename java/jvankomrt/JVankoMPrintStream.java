package jvankomrt;


import java.io.OutputStream;
import java.io.PrintStream;

public class JVankoMPrintStream extends PrintStream {

    private JVankoMPrintStream() {
        // never call
        super((OutputStream) null);
    }

    public static native PrintStream construct();


    @Override
    public void println(String s) {
        nativeWriteString(s);
        nativeWriteString("\n");
    }

    @Override
    public void print(String s) {
        nativeWriteString(s);
    }

    @Override
    public void write(int b) {
        nativeWrite(new byte[]{(byte) b}, 0, 1);
    }

    @Override
    public void write(byte[] bytes, int offset, int length) {
        nativeWrite(bytes, offset, length);
    }

    @Override
    public void flush() {}

    @Override
    public void close() {}

    @Override
    public boolean checkError() {
        return false;
    }

    private static native void nativeWrite(byte[] bytes, int offset, int length);
    private static native void nativeWriteString(String str);

}
