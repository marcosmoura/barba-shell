import { Battery } from './Battery';
import { Clock } from './Clock';

import * as styles from './Status.styles';

export const Status = () => {
  return (
    <div className={styles.status}>
      <Battery />
      <Clock />
    </div>
  );
};
