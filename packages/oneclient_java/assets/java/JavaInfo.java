import java.io.File;
import java.util.Locale;

public final class JavaInfo {
    private static final String[] CHECKED_PROPERTIES = new String[] {"os.arch", "java.version", "java.vendor", "java.home"};

    public static void main(String[] args) {
        int returnCode = 0;

        for (String key : CHECKED_PROPERTIES) {
            String property = System.getProperty(key);

            if (property != null) {
                System.out.println(key + "=" + property);
            } else {
                returnCode = 1;
            }
        }

        System.out.println("java.awt=" + hasClass("java.awt.Toolkit"));
        System.out.println("java.awt.natives=" + hasAwtNatives());
        System.out.println("java.jdk=" + hasCompiler());

        System.exit(returnCode);
    }

    /** Loads the class without initialising it, so no display is ever needed. */
    private static boolean hasClass(String name) {
        try {
            Class.forName(name, false, JavaInfo.class.getClassLoader());
            return true;
        } catch (Throwable ignored) {
            return false;
        }
    }

    private static boolean hasAwtNatives() {
        try {
            String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
            String[] required;

            if (os.contains("win")) {
                required = new String[] {"awt.dll", "fontmanager.dll"};
            } else if (os.contains("mac") || os.contains("darwin")) {
                required = new String[] {"libawt_lwawt.dylib", "libfontmanager.dylib"};
            } else {
                required = new String[] {"libawt_xawt.so", "libfontmanager.so"};
            }

            for (String name : required) {
                if (!findNative(name)) {
                    return false;
                }
            }

            return true;
        } catch (Throwable ignored) {
            return false;
        }
    }

    private static boolean findNative(String name) {
        File home = new File(System.getProperty("java.home", "."));
        File[] roots = new File[] {
            new File(home, "lib"),
            new File(home, "bin"),
            new File(new File(home, "lib"), System.getProperty("os.arch", "")),
        };

        for (File root : roots) {
            if (new File(root, name).isFile()) {
                return true;
            }
        }

        return false;
    }

    private static boolean hasCompiler() {
        try {
            return javax.tools.ToolProvider.getSystemJavaCompiler() != null;
        } catch (Throwable ignored) {
            return false;
        }
    }
}
