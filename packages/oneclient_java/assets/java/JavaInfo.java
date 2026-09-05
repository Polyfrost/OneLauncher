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

        System.out.println("java.jdk=" + hasCompiler());

        System.exit(returnCode);
    }

    private static boolean hasCompiler() {
        try {
            return javax.tools.ToolProvider.getSystemJavaCompiler() != null;
        } catch (Throwable ignored) {
            return false;
        }
    }
}
