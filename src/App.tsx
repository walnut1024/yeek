import { Providers } from "@/app/providers";
import { AppShell } from "@/app/shell";
import { ErrorBoundary } from "@/components/error-boundary";

function App() {
  return (
    <ErrorBoundary>
      <Providers>
        <AppShell />
      </Providers>
    </ErrorBoundary>
  );
}

export default App;
