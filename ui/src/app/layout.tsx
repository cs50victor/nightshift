import type { Metadata } from "next";
import "@fontsource-variable/inter";
import "@fontsource-variable/geist-mono";
import "@fontsource/geist-sans";
import "./globals.css";
import { Providers } from "./providers";

export const metadata: Metadata = {
  title: "Nightshift",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" suppressHydrationWarning>
      <body>
        <Providers>
          <div className="page">
            <section className="content">{children}</section>
          </div>
        </Providers>
      </body>
    </html>
  );
}
