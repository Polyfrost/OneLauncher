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

    private static boolean hasCompiler() {
        try {
            return javax.tools.ToolProvider.getSystemJavaCompiler() != null;
        } catch (Throwable ignored) {
            return false;
        }
    }
}
