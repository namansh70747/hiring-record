import type { Metadata } from 'next';
import './globals.css';
import './readability.css';

export const metadata: Metadata = {
  title: 'Hiring Record — Every decision has a history',
  description: 'A living record of the hiring bar, the people in flight, and the evidence behind every decision.',
};

export default function RootLayout({ children }: Readonly<{ children: React.ReactNode }>) {
  return <html lang="en"><body>{children}</body></html>;
}
